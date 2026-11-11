"""Bounded recipient-list import. Never sends mail or writes files."""
import base64,csv,io,re,zipfile
MAX_BYTES=2*1024*1024
def valid_email(value):
    parts=value.split('@')
    return len(value)<=254 and len(parts)==2 and bool(parts[0]) and '.' in parts[1] and not parts[1].startswith('.') and not parts[1].endswith('.') and not re.search(r'[\s<>,;"\x00-\x1f\x7f]',value)
def from_rows(rows):
    nonblank=[]
    for number,row in enumerate(rows,1):
        if number>10000:raise ValueError('Recipient file has too many rows (maximum 10,000).')
        if len(row)>100:raise ValueError('Recipient file has too many columns (maximum 100).')
        cells=[str(v).strip() if v is not None else '' for v in row]
        if any(cells):nonblank.append((number,cells))
    if not nonblank:raise ValueError('The recipient file is empty.')
    columns=[i for i,v in enumerate(nonblank[0][1]) if re.sub(r'[ _-]','',v.lower()) in ('email','emailaddress','recipient','recipients')]
    if len(columns)>1:raise ValueError('Use exactly one email column.')
    column=columns[0] if columns else 0
    if columns:nonblank=nonblank[1:]
    elif any(len(row)!=1 for _,row in nonblank):raise ValueError('Add an Email column header to a multi-column file.')
    result=[];seen=set()
    for number,row in nonblank:
        value=row[column] if column<len(row) else ''
        if not valid_email(value):raise ValueError(f'Invalid email address on row {number}. Correct the file and import it again.')
        if value.lower() not in seen:seen.add(value.lower());result.append(value)
    if not result or len(result)>100 or len(', '.join(result).encode('utf-8'))>8000:raise ValueError('Choose 1–100 distinct recipients (up to 8,000 characters total).')
    return result
def read(name,data):
    if not isinstance(name,str) or len(name)>255 or not name.lower().endswith(('.csv','.txt','.xlsx')):raise ValueError('Choose a CSV, TXT or XLSX recipient file.')
    if not isinstance(data,str) or len(data)>2800000:raise ValueError('Recipient files must be 2 MB or smaller.')
    try:raw=base64.b64decode(data,validate=True)
    except Exception:raise ValueError('Could not read the recipient file.')
    if len(raw)>MAX_BYTES:raise ValueError('Recipient files must be 2 MB or smaller.')
    if name.lower().endswith('.xlsx'):
        try:
            with zipfile.ZipFile(io.BytesIO(raw)) as archive:
                entries=archive.infolist()
                if len(entries)>1000 or sum(e.file_size for e in entries)>20*1024*1024:raise ValueError('The Excel file expands beyond the supported limit. Export its recipient list as CSV.')
            from openpyxl import load_workbook
            book=load_workbook(io.BytesIO(raw),read_only=True,data_only=False,keep_links=False)
            try:
                sheet=book.worksheets[0]
                if sheet.max_row and sheet.max_row>10000 or sheet.max_column and sheet.max_column>100:raise ValueError('Use a recipient sheet with at most 10,000 rows and 100 columns.')
                return from_rows(sheet.iter_rows(values_only=True))
            finally:book.close()
        except ValueError:raise
        except Exception:raise ValueError('Could not read the Excel recipient file. Save it as XLSX or CSV and try again.')
    try:
        text=raw.decode('utf-8-sig')
        rows=csv.reader(io.StringIO(text),strict=True) if name.lower().endswith('.csv') else ([line] for line in text.splitlines())
        return from_rows(rows)
    except (UnicodeError,csv.Error):raise ValueError('Use a UTF-8 CSV or TXT file with one email address per row.')
