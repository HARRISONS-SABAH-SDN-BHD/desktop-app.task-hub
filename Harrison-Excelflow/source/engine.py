"""Local FTG/DISC normalization and single-pass report generation."""
from __future__ import annotations
import calendar, csv, json, math, re, threading, time, sys
from functools import lru_cache
from copy import copy
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from datetime import date, datetime
from decimal import Decimal, InvalidOperation, ROUND_HALF_UP
from pathlib import Path
from openpyxl import Workbook, load_workbook
from openpyxl.cell import WriteOnlyCell
from openpyxl.styles import Alignment, Font, PatternFill, Border, Side, Protection, Color
from openpyxl.styles.fills import Fill
from openpyxl.xml.functions import fromstring
from openpyxl.utils import get_column_letter
from openpyxl.utils.datetime import from_excel

class Cancelled(Exception): pass
class InvalidData(Exception): pass

def selected_input_paths(paths,scope='Both',require_any=True):
    """Both means all provided inputs; one input is sufficient."""
    if scope not in ['Both','DISC','FTG']:raise InvalidData('Choose Both, DISC or FTG reports.')
    selected={k:v for k,v in paths.items() if k in ['DISC','FTG'] and str(v).strip() and (scope=='Both' or k==scope)}
    if require_any and not selected:
        raise InvalidData('Choose at least one DISC or FTG workbook.' if scope=='Both' else f'Choose a {scope} workbook, or select Both to process the available input.')
    return selected

def period_from_filename(path,kind=None):
    """Read the explicit ALL_<kind>_YYMM convention; years are 2000–2099."""
    from pathlib import PureWindowsPath
    filename=PureWindowsPath(str(path).strip()).name
    match=re.fullmatch(r'ALL_(DISC|FTG)_(\d{2})(\d{2})(?:\s*\(\d+\))?\.(xlsx|xlsm)',filename,re.IGNORECASE)
    if not match:raise InvalidData(f'{filename}: expected ALL_{kind or "DISC/FTG"}_YYMM.xlsx, for example ALL_{kind or "DISC"}_2610.xlsx. Rename the file or turn off automatic period detection.')
    found,yy,mm,_=match.groups()
    if kind and found.upper()!=kind:raise InvalidData(f'{filename} is a {found.upper()} file. Select it in the {found.upper()} section, not {kind}.')
    month=int(mm)
    if not 1<=month<=12:raise InvalidData(f'{filename}: {mm} is not a valid month (01–12).')
    return month,2000+int(yy)

def period_from_inputs(paths,require_both=True):
    periods={kind:period_from_filename(path,kind) for kind,path in paths.items() if str(path).strip()}
    if require_both and set(periods)!={'DISC','FTG'}:raise InvalidData('Select both DISC and FTG raw input files to detect the reporting period.')
    if not periods:return None
    if len(set(periods.values()))!=1:
        labels=', '.join(f'{k}: {calendar.month_name[m]} {y}' for k,(m,y) in periods.items())
        raise InvalidData(f'Input filenames have different reporting periods ({labels}). Select two files for the same month and year.')
    return next(iter(periods.values()))

def check_cancel(cancel):
    if cancel and cancel.is_set(): raise Cancelled('Cancelled. Original inputs were not changed.')

def text(v):
    if v is None: return ''
    if isinstance(v, bool): return str(v)
    if isinstance(v, (int, float, Decimal)):
        if isinstance(v, float) and not math.isfinite(v): return ''
        return format(Decimal(str(v)), 'f').rstrip('0').rstrip('.') if Decimal(str(v)) % 1 else str(int(v))
    return str(v).strip()

def identifier(v):
    s=text(v)
    if re.fullmatch(r'\d+\.0+', s): return s.split('.')[0]
    return s

def number(v, required=True):
    if isinstance(v, bool): raise ValueError('A TRUE/FALSE value is not a number.')
    s=text(v).replace('\u00a0','').replace(' ','')
    if not s:
        if required: raise ValueError('Required numeric value is blank.')
        return None
    neg=s.startswith('(') and s.endswith(')')
    if neg:s=s[1:-1]
    if ',' in s:
        if not re.fullmatch(r'[+-]?\d{1,3}(,\d{3})+(\.\d+)?',s):
            raise ValueError('Ambiguous separator. Use 1,234.56 or 1234.56.')
        s=s.replace(',','')
    try: n=Decimal(s)
    except InvalidOperation: raise ValueError('Not a valid number.')
    if not n.is_finite(): raise ValueError('Number must be finite.')
    return -n if neg else n

def whole(v):
    n=number(v)
    if n != n.to_integral_value():raise ValueError('Expected a whole number; value was not rounded.')
    return int(n)

def document_date(v, month, year, repair=True, epoch=None):
    """Accept typed dates, serials and explicit date strings. Never invent a year."""
    if isinstance(v,bool) or v is None:raise ValueError('Document date is blank or invalid.')
    candidates=[]
    if isinstance(v, datetime): candidates=[v.date()]
    elif isinstance(v,date):candidates=[v]
    elif isinstance(v,(int,float)):
        if not 1<=v<=2958465:raise ValueError('Excel date serial is outside the supported range.')
        dt=from_excel(v,epoch) if epoch else from_excel(v)
        if not isinstance(dt,(datetime,date)):raise ValueError('Date serial has no calendar date.')
        candidates=[dt.date() if isinstance(dt,datetime) else dt]
    else:
        s=text(v)
        # A time suffix is allowed; unexplained trailing characters are rejected.
        s=re.sub(r'(?:[ T]\d{1,2}:\d{2}(?::\d{2}(?:\.\d+)?)?)$', '',s).strip()
        m=re.fullmatch(r'(\d{1,4})[/.\-](\d{1,2})[/.\-](\d{1,4})',s)
        if m:
            a,b,c=m.groups()
            if len(a)==4:
                try:candidates=[date(int(a),int(b),int(c))]
                except ValueError:pass
            elif len(c)==4:
                for dd,mm in [(int(a),int(b)),(int(b),int(a))]:
                    try:candidates.append(date(int(c),mm,dd))
                    except ValueError:pass
            else:raise ValueError('A four-digit year is required.')
        else:
            for fmt in ['%d %B %Y','%d %b %Y','%B %d, %Y','%b %d, %Y']:
                try:candidates.append(datetime.strptime(s,fmt).date());break
                except ValueError:pass
    original=candidates[0] if candidates else None
    # Recovery of already misinterpreted Excel dates uses selected period as authority.
    if repair and isinstance(v,(datetime,date,int,float)):
        for d in list(candidates):
            if d.day<=12:
                try:candidates.append(date(d.year,d.day,d.month))
                except ValueError:pass
    matching={d for d in candidates if d.month==month and d.year==year}
    if len(matching)!=1:raise ValueError(f'Date cannot be resolved to {calendar.month_name[month]} {year}. Check the source date or selected period.')
    chosen=matching.pop()
    return chosen, chosen!=original

def header_key(v):return re.sub(r'[^a-z0-9]','',text(v).lower())

ALIASES={
 'branch':['inventsite_id','branch code','branch id','branch','inventsite_name','branch name'],
 'item':['item No','item no.','item number','item code'], 'item_id':['item_id','item id'],
 'product':['Product Name','product description','item name'],
 'date':['Document Date','document_date','doc date','invoice date'],
 'document':['Document No','document number','doc no','invoice no'],
 'customer':['Customer Account','customer id','customer code'],
 'customer_name':['Customer Name'], 'promotion':['promotion_code','promotion code'],
 'description':['IPC Description','promotion description'],
 'qty':['invoiceqty','Invoice Qty','invoice quantity'], 'unit':['Unit','invoice unit'],
 'gross':['gross_amount','gross amount'], 'discount':['discount_amount','discount amount','DISC amount'],
 'pgc':['itemgroup_id','PGC','item group id'], 'foc_qty':['FOC Qty','free qty'],
 'foc_unit':['Unit.1','FOC Unit','free unit'], 'foc_amount':['FOC Amount','free amount'],
 'month':['invoicemonth','invoice month'], 'year':['invoiceyear','invoice year']}

@dataclass
class Options:
    month:int
    year:int
    mode:str='Both'
    money_places:int=2
    quantity_places:int=2
    repair_dates:bool=True
    factor:Decimal=Decimal('0.88')

@dataclass
class Batch:
    rows:dict=field(default_factory=dict)
    issues:list=field(default_factory=list)
    stats:dict=field(default_factory=dict)
    preview:list=field(default_factory=list)
    def issue(self,kind,row,column,value,message,severity='ERROR'):
        self.issues.append([severity,kind,row,column,text(value),message])
    @property
    def errors(self):return sum(i[0]=='ERROR' for i in self.issues)

def read_input(path,kind,opt,settings,batch,progress=lambda *_:None,cancel=None):
    check_cancel(cancel)
    w=load_workbook(path,read_only=True,data_only=False)
    try:
        aliases={**ALIASES,**settings.get('column_aliases',{})}
        # Prefer Sheet1; otherwise accept the sole nonempty sheet with recognizable headers.
        choices=[]
        for s in w.worksheets:
            hs=next(s.iter_rows(values_only=True),())
            if any(header_key(x) in [header_key(a) for a in aliases['date']] for x in hs):choices.append(s)
        if len(choices)!=1:raise InvalidData(f'{kind}: expected one data sheet with Document Date in row 1; found {len(choices)}.')
        s=choices[0]; it=s.iter_rows(values_only=True); raw=list(next(it)); headers=[]; counts=Counter()
        for h in raw:
            key=header_key(h); counts[key]+=1
            headers.append(key if counts[key]==1 else key+str(counts[key]))
        ix={}
        for dest,names in aliases.items():
            for name in names:
                key=header_key(name)
                if key in headers:ix[dest]=headers.index(key);break
        # The second raw "Unit" column belongs to FOC, not invoice quantity.
        if 'foc_unit' not in ix and 'unit2' in headers:ix['foc_unit']=headers.index('unit2')
        required=['branch','product','date','document','customer','customer_name','promotion','description','qty','unit','gross','discount','pgc']
        if kind=='FTG':required+=['foc_qty','foc_amount','foc_unit']
        missing=[x for x in required if x not in ix]
        if missing or not any(x in ix for x in ['item','item_id']):raise InvalidData(f'{kind}: missing columns: {", ".join(missing or ["item/item_id"])}. Column aliases can be edited in settings.json.')
        known=set(ix.values()); extras=[text(raw[j]) for j in range(len(raw)) if j not in known]
        branches=settings['branches']; look={}
        for code,info in branches.items():
            for key in [code,info[0],info[1],code[-2:]]:look[text(key).upper()]=code
        agency_ids=defaultdict(list)
        for pgc,name in settings.get('agencies',{}).items():agency_ids[text(name).casefold()].append(int(pgc))
        records=[]; repaired=0; date_types=Counter(); numeric_strings=0; blank_rows=0
        for rownum,values in enumerate(it,2):
            if rownum%1000==0:
                check_cancel(cancel);progress(f'Validating {kind}: {rownum-1:,} rows',min(99,int((rownum-1)/max(1,s.max_row-1)*100)) if s.max_row else None)
            if not any(v is not None and text(v) for v in values):blank_rows+=1;continue
            r={k:values[j] if j<len(values) else None for k,j in ix.items()}; out={}; before=len(batch.issues)
            for k,v in r.items():
                if isinstance(v,str) and v.startswith('='):batch.issue(kind,rownum,k,v,'Formula in raw data. Export calculated values instead.');continue
                try:
                    if k=='date':
                        out[k],swapped=document_date(v,opt.month,opt.year,opt.repair_dates,w.epoch);repaired+=int(swapped);date_types[type(v).__name__]+=1
                        if len([p for p in batch.preview if p[0]==kind])<8:batch.preview.append([kind,rownum,text(v),out[k].strftime('%d/%m/%Y'),'Period-based correction' if swapped else 'Parsed'])
                    elif k in ['gross','discount','qty','foc_qty','foc_amount']:
                        out[k]=number(v);numeric_strings+=int(isinstance(v,str))
                    elif k in ['pgc','month','year']:
                        matches=agency_ids.get(text(v).casefold(),[]) if k=='pgc' else []
                        if len(matches)>1:raise ValueError('Agency name maps to multiple PGC IDs. Supply the numeric PGC.')
                        out[k]=matches[0] if matches else whole(v)
                        if k=='pgc' and out[k]<=0:raise ValueError('PGC must be a positive whole number.')
                        if k=='month' and out[k]!=opt.month:raise ValueError('Invoice month disagrees with selected month.')
                        if k=='year' and out[k]!=opt.year:raise ValueError('Invoice year disagrees with selected year.')
                    elif k=='branch':
                        key=text(v).upper()
                        if key not in look:raise ValueError('Unknown branch code/name. Add it to settings.json before processing.')
                        out[k]=look[key]
                    elif k in ['item','item_id','document','customer']:
                        out[k]=identifier(v)
                        if isinstance(v,(int,float)) and len(out[k].lstrip('-0'))>15:out.setdefault('_id_warnings',[]).append(k)
                    else:out[k]=text(v)
                except (ValueError,OverflowError,TypeError) as e:batch.issue(kind,rownum,text(raw[ix[k]]),v,str(e))
            for k in ['document','customer','product','customer_name','unit']:
                if not out.get(k):batch.issue(kind,rownum,k,r.get(k),'Required value is blank.')
            if not out.get('item') and not out.get('item_id'):batch.issue(kind,rownum,'item',None,'Both item number and fallback item ID are blank.')
            out['promotion']=out.get('promotion','').strip()
            if out['promotion'].upper() in ['', 'N/A','NA','NULL','NONE']:out['promotion']='N/A'
            out['_row']=rownum
            if len(batch.issues)==before:records.append(out)
        # Preserve the original rule: use item_id for the entire affected PGC.
        fallback_groups={r['pgc'] for r in records if not r.get('item')}
        for r in records:
            if r['pgc'] in fallback_groups:
                if not r.get('item_id'):batch.issue(kind,r['_row'],'item_id',None,'This PGC requires fallback item IDs, but one is blank.')
            for source_key in r.get('_id_warnings',[]):
                batch.issue(kind,r['_row'],source_key,r.get(source_key),'Long numeric ID preserved as stored. Excel may already have rounded it; verify against the source system.','WARNING')
        batch.rows[kind]=records
        valid_row_numbers={r['_row'] for r in records}
        batch.stats[kind]={'source_rows':len(records)+len({i[2] for i in batch.issues if i[1]==kind and i[0]=='ERROR' and i[2] not in valid_row_numbers}), 'valid_rows':len(records),'date_types':dict(date_types),'period_date_corrections':repaired,'numeric_text_converted':numeric_strings,'blank_rows_ignored':blank_rows,'unused_columns':extras,'promotion_counts':dict(Counter(r['promotion'][:4] for r in records))}
    finally:w.close()

DISC=[('ITEM NO','item'),('PRODUCT NAME','product'),('DOCUMENT DATE','date'),('DOCUMENT NO','document'),('CUSTOMER ACCOUNT','customer'),('CUSTOMER NAME','customer_name'),('PROMOTION CODE','promotion'),('IPC DESCRIPTION','description'),('INVOICE QTY','qty'),('UNIT','unit'),('GROSS AMOUNT','gross'),('DISC AMOUNT','discount'),('DISC AMOUNT (2)','discount2'),('PGC','pgc'),('BRANCH CODE','branch'),('BRANCH NAME','branch_name')]
FTG=[('Item No.','item'),('Product Name','product'),('Document Date','date'),('Document No.','document'),('Customer Account','customer'),('Customer Name','customer_name'),('Promotion Code','promotion'),('IPC Description','description'),('Invoice Qty','qty'),('Unit','unit'),('DISC Amount','discount'),('DISC Amount (2)','discount2'),('FOC Qty','foc_qty'),('Unit.1','foc_unit'),('FOC Amount','foc_amount'),('Branch Code','branch'),('itemgroup_id','pgc')]

def selected(rows,kind,special,opt,settings):
    prefix=('IPCF' if kind=='DISC' else 'IPCD') if special else ('IPCD' if kind=='DISC' else 'IPCF')
    result=[]; quantum=Decimal(1).scaleb(-opt.money_places)
    for raw in rows:
        if not (raw['promotion'].upper().startswith(prefix) or (not special and raw['promotion']=='N/A')):continue
        r=raw.copy();r['branch_name']=settings['branches'][r['branch']][0]
        if kind=='DISC':
            if r['gross']>0:r['discount']=abs(r['discount'])
            elif r['gross']<0:r['discount']=-abs(r['discount'])
        else:
            sign=1 if r['qty']>=0 else -1
            for k in ['discount','foc_qty','foc_amount']:r[k]=abs(r[k])*sign
        r['discount2']=(r['discount']*opt.factor).quantize(quantum,rounding=ROUND_HALF_UP) if r['pgc']==160 else None
        result.append(r)
    fallback_groups={r['pgc'] for r in result if not r.get('item')}
    for r in result:
        if r['pgc'] in fallback_groups:r['item']=r['item_id']
    return result

def sheet_columns(kind,pgc=None):
    cols=list(DISC if kind=='DISC' else FTG)
    if pgc is not None:
        cols=[(h,k) for h,k in cols if k not in (['pgc','branch'] if kind=='DISC' else ['pgc']) and (k!='discount2' or pgc==160)]
        if kind=='DISC':cols=[({'GROSS AMOUNT':'GROSS AMT','DISC AMOUNT':'DISC AMT','BRANCH NAME':'BRANCH'}.get(h,h),k) for h,k in cols]
        if kind=='DISC':cols.insert(len(cols)-1,(None,'_spacer'))
    return cols

@lru_cache(maxsize=1)
def report_styles():
    base=Path(sys.executable).parent if getattr(sys,'frozen',False) else Path(__file__).resolve().parent.parent
    profiles=json.loads((base/'report_styles.json').read_text(encoding='utf-8'))
    def unpack(style):
        result={k:cls.from_tree(fromstring(style[k])) for k,cls in [('font',Font),('fill',Fill),('border',Border),('alignment',Alignment),('protection',Protection)]}
        # The original line patterns are retained, with explicit grey instead of automatic black.
        for side in ['left','right','top','bottom','diagonal','vertical','horizontal']:
            edge=getattr(result['border'],side,None)
            if edge is not None and edge.style:edge.color=Color(rgb='FF808080')
        result['number_format']=style['number_format'];return result
    for p in profiles.values():
        for key in ['header_styles','total_styles','metadata_styles']:p[key]=[unpack(x) for x in p[key]]
        p['data_styles']=[{k:unpack(v) for k,v in ds.items()} for ds in p['data_styles']]
    return profiles

def apply_report_style(cell,style):
    workbook=cell.parent.parent
    if not hasattr(workbook,'_report_style_cache'):workbook._report_style_cache={}
    cache=workbook._report_style_cache
    key=id(style)
    if key not in cache:
        for k,v in style.items():setattr(cell,k,v)
        cache[key]=copy(cell._style)
    else:cell._style=copy(cache[key])

def save_agencies(path,agencies):
    """Validate before atomically persisting edits; preserve other settings."""
    cleaned={}
    for code,name in agencies.items():
        key=str(code).strip()
        if not re.fullmatch(r'[1-9]\d*',key):raise InvalidData('IG numbers must be positive whole numbers without leading zeros.')
        if len(key)>8:raise InvalidData('IG numbers must have at most 8 digits.')
        if not isinstance(name,str) or not name.strip():raise InvalidData(f'IG {key} needs a name.')
        if key in cleaned:raise InvalidData(f'Duplicate IG number {key}.')
        cleaned[key]=name.strip()
    dest=Path(path);data=load_settings(dest);data['agencies']={k:cleaned[k] for k in sorted(cleaned,key=int)}
    temp=dest.with_suffix('.json.tmp');temp.write_text(json.dumps(data,indent=2,ensure_ascii=False),encoding='utf-8');temp.replace(dest)
    return data['agencies']

def write_sheet(w,name,rows,kind,opt,settings,branch,pgc=None,cancel=None):
    s=w.create_sheet(name);cols=sheet_columns(kind,pgc)
    profile=report_styles()[kind+('_main' if pgc is None else '_p160' if pgc==160 else '_pgc')]
    for i,(h,k) in enumerate(cols,1):
        width=max([len(str(h or ''))]+[len(str(r.get(k) or '')) for r in rows])+2
        if k=='date':width=15
        if k=='_spacer':width=2
        s.column_dimensions[get_column_letter(i)].width=max(profile['widths'][i-1],width)
    s.sheet_view.showGridLines=profile['gridlines'];s.freeze_panes='C8';s.auto_filter.ref=f'A7:{get_column_letter(len(cols))}{7+len(rows)}'
    title='Trade Discount Report' if kind=='DISC' else 'Trade Offer Report'
    agency='ALL' if pgc is None else settings.get('agencies',{}).get(str(pgc),f'IG {pgc} (unmapped)')
    metadata=[title,settings.get('company',''),f'Branch : {branch}',f'Date : 01/{opt.month:02}/{opt.year} - {calendar.monthrange(opt.year,opt.month)[1]:02}/{opt.month:02}/{opt.year}',f'Agency : {agency}']
    for i,v in enumerate(metadata):
        c=WriteOnlyCell(s,v);c.data_type='s';apply_report_style(c,profile['metadata_styles'][i]);c.alignment=Alignment(horizontal='left',vertical=c.alignment.vertical)
        s.merged_cells.add(f'A{i+1}:B{i+1}');s.append([c])
    s.append([])
    header=[]
    for j,(h,k) in enumerate(cols):
        c=WriteOnlyCell(s,h);apply_report_style(c,profile['header_styles'][j]);header.append(c)
    s.append(header); totals=defaultdict(Decimal)
    for n,r in enumerate(rows):
        if n%1000==0:check_cancel(cancel)
        cells=[]
        for j,(h,k) in enumerate(cols):
            v=r.get(k);c=WriteOnlyCell(s)
            variants=profile['data_styles'][j];sign='negative' if isinstance(v,Decimal) and v<0 else 'zero' if isinstance(v,Decimal) and v==0 else 'positive'
            apply_report_style(c,variants.get(sign,variants['default']))
            if isinstance(v,Decimal):
                c.value=int(v) if v==v.to_integral_value() else float(v)
                places=opt.quantity_places if k in ['qty','foc_qty'] else opt.money_places
                fmt=c.number_format
                if fmt=='General':fmt='0.00;('+ '0.00'+')'
                c.number_format=re.sub(r'0\.0+', '0'+('.'+'0'*places if places else ''),fmt)
                if v<0 and ';' not in c.number_format:c.number_format+=';('+c.number_format+')'
                totals[k]+=v
            elif isinstance(v,date):
                c.value=v;c.number_format='d/m/yyyy';alignment=copy(c.alignment);alignment.horizontal='left';c.alignment=alignment
            elif isinstance(v,str):c.value=v;c.data_type='s';c.number_format='@'
            else:c.value=v
            cells.append(c)
        s.append(cells)
    totalrow=[]
    for j,(h,k) in enumerate(cols,1):
        c=WriteOnlyCell(s,'TOTAL' if j==1 else None)
        apply_report_style(c,profile['total_styles'][j-1])
        if k in ['gross','discount','discount2','foc_amount','qty','foc_qty']:
            # Materialized totals remain available in Excel previews without recalculation.
            v=totals[k];c.value=int(v) if v==v.to_integral_value() else float(v)
            places=opt.quantity_places if k in ['qty','foc_qty'] else opt.money_places;fmt='#,##0'+('.'+'0'*places if places else '')
            c.number_format=fmt+';('+fmt+');"-"';c.border=Border(top=Side(style='thin',color='FF808080'),bottom=Side(style='double',color='FF808080'))
        else:c.border=Border()
        totalrow.append(c)
    s.append(totalrow)
    return {k:str(v) for k,v in totals.items()}

def write_issues(path,batch):
    with open(path,'w',encoding='utf-8-sig',newline='') as f:
        writer=csv.writer(f);writer.writerow(['Severity','Input','Excel row','Column','Raw value','Message'])
        for row in batch.issues:
            # Keep exported raw values from becoming spreadsheet formulas.
            writer.writerow([("'"+v if isinstance(v,str) and v.startswith(('=','+','-','@')) else v) for v in row])

def generate(batch,opt,settings,destination,progress=lambda *_:None,cancel=None):
    if batch.errors:raise InvalidData(f'{batch.errors:,} validation errors. Resolve them before generating reports.')
    if not batch.rows:raise InvalidData('No input workbooks were provided.')
    root=Path(destination)/('DISC & FTG Reports '+datetime.now().strftime('%Y%m%d_%H%M%S_%f'));root.mkdir(parents=True)
    summary={'period':f'{opt.year}-{opt.month:02}','settings':{'money_places':opt.money_places,'quantity_places':opt.quantity_places,'p160_multiplier':str(opt.factor),'date_recovery':opt.repair_dates},'inputs':batch.stats,'files':[],'empty_reports':[],'status':'running'}
    tasks=[]
    for special in ([False,True] if opt.mode=='Both' else [opt.mode=='Special']):
        dirname=f'Formatted {calendar.month_name[opt.month]} {opt.year}'+(' (DISC with IPCF, FTG with IPCD)' if special else '')
        for kind in ['DISC','FTG']:
            if kind not in batch.rows:continue
            records=selected(batch.rows[kind],kind,special,opt,settings);groups=defaultdict(list)
            for r in records:groups[r['branch']].append(r)
            if not records:summary['empty_reports'].append({'mode':'Special' if special else 'Normal','kind':kind,'reason':'No matching promotion codes.'})
            for code,rows in sorted(groups.items()):tasks.append((dirname,kind,code,rows))
    try:
        for i,(dirname,kind,code,rows) in enumerate(tasks):
            check_cancel(cancel);branch,abbr=settings['branches'][code];progress(f'Writing {i+1}/{len(tasks)}: {abbr} {kind} — {len(rows):,} rows',int(i/len(tasks)*100))
            path=root/dirname/kind/f'{code[-2:]} {abbr} {kind} {opt.month:02}{opt.year%100:02}.xlsx';path.parent.mkdir(parents=True,exist_ok=True)
            w=Workbook(write_only=True)
            totals=write_sheet(w,abbr,rows,kind,opt,settings,branch,cancel=cancel)
            if kind=='FTG':
                ig=w.create_sheet('IG');ig.append(['PGC','DESC'])
                for k,v in settings.get('agencies',{}).items():
                    c=WriteOnlyCell(ig,v);c.data_type='s';ig.append([int(k),c])
            bypgc=defaultdict(list)
            for r in rows:bypgc[r['pgc']].append(r)
            for pgc,rs in sorted(bypgc.items()):write_sheet(w,f'P{pgc}',rs,kind,opt,settings,branch,pgc,cancel)
            check_cancel(cancel);w.save(path)
            summary['files'].append({'file':str(path.relative_to(root)),'kind':kind,'branch':code,'rows':len(rows),'totals':totals})
        summary['status']='complete';progress('Reports complete',100)
    except Exception as e:
        summary['status']='cancelled' if isinstance(e,Cancelled) else 'failed';summary['error']=str(e);e.output_dir=root;raise
    finally:
        (root/'run_summary.json').write_text(json.dumps(summary,indent=2),encoding='utf-8')
        if batch.issues:write_issues(root/'validation_issues.csv',batch)
    return root,summary

def load_settings(path):
    data=json.loads(Path(path).read_text(encoding='utf-8-sig'))
    for code,info in data['branches'].items():
        if not re.fullmatch(r'[A-Z]\d{2}',code) or len(info)!=2 or not all(isinstance(v,str) and v.strip() for v in info):raise InvalidData('Invalid branch mapping in settings.json.')
        if re.search(r'[\\/:*?"<>|]',info[1]) or len(info[1])>20:raise InvalidData('Branch abbreviation must be a safe, short filename/sheet name.')
    return data
