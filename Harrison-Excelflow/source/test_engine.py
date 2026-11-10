"""Run with: python -m unittest discover -s source (from app folder)."""
import tempfile,threading,unittest
from pathlib import Path
from datetime import date,datetime
from decimal import Decimal
from openpyxl import Workbook
from engine import *

class EngineTests(unittest.TestCase):
 def test_completion_sound_once_and_nonblocking(self):
  from app import App,ASSETS
  from types import SimpleNamespace
  from unittest.mock import patch
  import winsound
  from completion_sound import play_completion_sound
  fake=SimpleNamespace()
  with patch('app.play_completion_sound') as play:
   App.notify_completed(fake);App.notify_completed(fake)
   play.assert_called_once_with(ASSETS/'completion.wav')
   fake._completion_sound_played=False;App.notify_completed(fake)
   self.assertEqual(play.call_count,2)
  with patch('winsound.PlaySound') as play:
   play_completion_sound('completion.wav')
   self.assertTrue(play.call_args.args[1] & winsound.SND_ASYNC)
   self.assertTrue(play.call_args.args[1] & winsound.SND_NODEFAULT)
  with patch('winsound.PlaySound',side_effect=RuntimeError('Audio unavailable')):
   play_completion_sound('completion.wav')
 def test_readable_run_durations(self):
  from run_timing import format_duration,timing_record,timing_label
  self.assertEqual(format_duration(7.47),'7.47 seconds');self.assertEqual(format_duration(65.12),'1 minute 5.12 seconds');self.assertEqual(format_duration(3661.25),'1 hour 1 minute 1.25 seconds');self.assertEqual(format_duration(59.999),'1 minute')
  self.assertEqual(format_duration(1),'1.00 second');self.assertEqual(format_duration(120),'2 minutes');self.assertEqual(format_duration(90061),'1 day 1 hour 1 minute 1.00 second');self.assertEqual(format_duration(172800),'2 days')
  record=timing_record(7.47,34.82);self.assertEqual(record['total_seconds'],42.29);self.assertIn('Report generation: 34.82 seconds',timing_label(record))
  validation_only=timing_record(.12);self.assertIsNone(validation_only['processing_seconds']);self.assertIn('Report generation: Not run',timing_label(validation_only))
 def test_rounded_choices_share_state_and_block_disabled_input(self):
  from ui_widgets import ChoiceButton,ChoiceGroup
  from types import SimpleNamespace
  class Variable:
   def __init__(self):self.value='Both'
   def get(self):return self.value
   def set(self,value):self.value=value
  var=Variable();button=SimpleNamespace(_state='normal',variable=var,value='Normal',focus_set=lambda:None)
  ChoiceButton.invoke(button);self.assertEqual(var.get(),'Normal')
  button._state='disabled';button.value='Special';ChoiceButton.invoke(button);self.assertEqual(var.get(),'Normal')
  buttons=[SimpleNamespace(_state='normal',focus_set=lambda:None) for _ in range(3)]
  group=SimpleNamespace(variable=var,values=['Both','Normal','Special'],buttons=buttons)
  ChoiceGroup.move(group,1);self.assertEqual(var.get(),'Special');ChoiceGroup.move(group,1);self.assertEqual(var.get(),'Both')
  buttons[1]._state='disabled';ChoiceGroup.move(group,1);self.assertEqual(var.get(),'Both')
 def test_completed_progress_bar_stays_full_when_idle(self):
  from app import App
  from types import SimpleNamespace
  from queue import Queue
  class Widget:
   value=0
   def stop(self):self.value=0
   def configure(self,**kw):
    if 'value' in kw:self.value=kw['value']
  class Text:
   def set(self,value):self.value=value
  fake=SimpleNamespace(q=Queue(),busy=True,progress_value=0,progress=Widget(),progress_text=Text(),status=Text(),detail=Text(),cancel_button=Widget(),open_button=Widget(),controls=[],sync_period=lambda:None,append_log=lambda _:None,after=lambda *_:None)
  fake.set_progress=lambda n:App.set_progress(fake,n)
  fake.poll=lambda:None
  fake.notify_completed=lambda:None
  fake.q.put(('complete',Path('.'),{'files':[],'empty_reports':[]}));fake.q.put(('idle',))
  App.poll(fake);self.assertEqual(fake.progress.value,100);self.assertEqual(fake.progress_text.value,'100%');self.assertFalse(fake.busy)
 def test_worker_reports_progress_before_completion(self):
  from app import App
  from types import SimpleNamespace
  from queue import Queue
  import json
  headers=['inventsite_id','item No','Product Name','Document Date','Document No','Customer Account','Customer Name','promotion_code','IPC Description','Invoice Qty','Unit','gross_amount','discount_amount','itemgroup_id']
  row=['C20','00123','Product','13/10/2026','DOC','C001','Customer','N/A','Offer',1.25,'EA',12.50,1.25,160]
  settings={'branches':{'C20':['KOTA KINABALU','KK']},'agencies':{'160':'KAO'},'company':'Test'}
  with tempfile.TemporaryDirectory() as temp:
   path=Path(temp)/'ALL_DISC_2610.xlsx';w=Workbook();w.active.append(headers)
   for _ in range(1005):w.active.append(row)
   w.save(path)
   fake=SimpleNamespace(q=Queue(),cancel=threading.Event())
   App.work(fake,{'DISC':path},Options(10,2026),settings,Path(temp)/'reports',True)
   events=list(fake.q.queue);self.assertTrue(any(e[0]=='complete' for e in events),events[-3:])
   complete=next(e for e in events if e[0]=='complete');timings=complete[2]['timings']
   self.assertGreaterEqual(timings['validation_seconds'],0);self.assertGreaterEqual(timings['processing_seconds'],0);self.assertGreaterEqual(timings['total_seconds'],timings['validation_seconds'])
   saved=json.loads((complete[1]/'run_summary.json').read_text());self.assertEqual(saved['timings'],timings)
   values=[e[2] for e in events if e[0]=='progress' and e[2] is not None]
   self.assertEqual(values[0],0);self.assertTrue(any(0<v<30 for v in values));self.assertEqual(values,sorted(values));self.assertLessEqual(max(values),99)
   validation=SimpleNamespace(q=Queue(),cancel=threading.Event());App.work(validation,{'DISC':path},Options(10,2026),settings,Path(temp)/'reports',False)
   ready=next(e for e in validation.q.queue if e[0]=='ready');self.assertIsNone(ready[1]['processing_seconds']);self.assertGreaterEqual(ready[1]['validation_seconds'],0)
 def test_progress_percentage_and_monotonicity(self):
  from app import App
  from types import SimpleNamespace
  class Bar:
   def stop(self):pass
   def configure(self,**kw):self.value=kw['value']
  class Text:
   def set(self,value):self.value=value
  fake=SimpleNamespace(progress_value=0,progress=Bar(),progress_text=Text())
  for value,expected in [(-5,0),(24.6,25),(10,25),(99,99),(150,100)]:
   App.set_progress(fake,value);self.assertEqual(fake.progress_value,expected);self.assertEqual(fake.progress_text.value,f'{expected}%');self.assertEqual(fake.progress.value,expected)
  fake.progress_value=0;App.set_progress(fake,0);self.assertEqual(fake.progress_text.value,'0%')
 def test_menu_navigation_preserves_report(self):
  from app import App
  from types import SimpleNamespace
  class Frame:
   visible=False
   def pack(self,**kw):self.visible=True
   def pack_forget(self):self.visible=False
  report=Frame();menu=Frame();details=Frame();report.visible=True;files=object()
  fake=SimpleNamespace(busy=False,report_frame=report,menu_frame=menu,details_frame=details,files=files,title=lambda _:None)
  App.show_main_menu(fake);self.assertEqual(fake.current_screen,'menu');self.assertTrue(menu.visible);self.assertFalse(report.visible)
  App.show_report(fake);self.assertEqual(fake.current_screen,'report');self.assertFalse(menu.visible);self.assertTrue(report.visible);self.assertIs(fake.files,files)
  fake.busy=True;App.show_main_menu(fake);self.assertEqual(fake.current_screen,'report')
  App.show_run_details(fake);self.assertEqual(fake.current_screen,'details');self.assertTrue(details.visible);self.assertFalse(report.visible)
  App.show_report(fake);self.assertEqual(fake.current_screen,'report');self.assertFalse(details.visible);self.assertIs(fake.files,files)
  fake.busy=False;App.show_run_details(fake);App.show_main_menu(fake);self.assertFalse(details.visible);self.assertTrue(menu.visible)
 def test_optional_input_selection(self):
  paths={'DISC':'ALL_DISC_2610.xlsx','FTG':''}
  self.assertEqual(selected_input_paths(paths),{'DISC':'ALL_DISC_2610.xlsx'})
  self.assertEqual(period_from_inputs(selected_input_paths(paths),False),(10,2026))
  self.assertEqual(selected_input_paths({'DISC':'','FTG':'ALL_FTG_2610.xlsx'}),{'FTG':'ALL_FTG_2610.xlsx'})
  self.assertEqual(selected_input_paths({'DISC':'ALL_DISC_2610.xlsx','FTG':'ALL_FTG_2701.xlsx'},'DISC'),{'DISC':'ALL_DISC_2610.xlsx'})
  for paths,scope in [({'DISC':'','FTG':''},'Both'),({'DISC':'a','FTG':''},'FTG'),({'DISC':'','FTG':'b'},'DISC')]:
   with self.assertRaises(InvalidData):selected_input_paths(paths,scope)
 def test_single_input_generation(self):
  settings={'branches':{'C20':['KOTA KINABALU','KK']},'agencies':{'160':'KAO'},'company':'Test'}
  raw={'item':'00123','product':'Product','date':date(2026,10,13),'document':'D001','customer':'C001','customer_name':'Customer','promotion':'N/A','description':'Offer','pgc':160,'branch':'C20','discount':Decimal('1.25'),'gross':Decimal('10'),'qty':Decimal('2'),'unit':'EA','foc_qty':Decimal('1'),'foc_unit':'EA','foc_amount':Decimal('2')}
  with tempfile.TemporaryDirectory() as temp:
   for kind in ['DISC','FTG']:
    batch=Batch(rows={kind:[raw]},stats={kind:{'source_rows':1}})
    root,summary=generate(batch,Options(10,2026),settings,temp)
    self.assertEqual(len(summary['files']),1);self.assertEqual(summary['files'][0]['kind'],kind)
    self.assertFalse(any((root/r['file']).parent.name!=kind for r in summary['files']))
    w=load_workbook(root/summary['files'][0]['file']);self.assertEqual(w.worksheets[0]['A8'].value,'00123');self.assertEqual(w.worksheets[0]['A3'].value,'Branch : KOTA KINABALU');w.close()
 def test_filename_period(self):
  for month in range(1,13):
   self.assertEqual(period_from_filename(f'ALL_DISC_26{month:02}.xlsx','DISC'),(month,2026))
  self.assertEqual(period_from_filename('all_ftg_2701.xlsm','FTG'),(1,2027))
  self.assertEqual(period_from_filename(r'C:\exports\ALL_DISC_9912 (2).xlsx','DISC'),(12,2099))
  self.assertEqual(period_from_inputs({'DISC':'ALL_DISC_2610.xlsx','FTG':'ALL_FTG_2610.xlsx'}),(10,2026))
  self.assertEqual(period_from_inputs({'DISC':'ALL_DISC_2610.xlsx','FTG':''},False),(10,2026))
  self.assertIsNone(period_from_inputs({'DISC':'','FTG':''},False))
 def test_filename_period_rejections(self):
  for path in ['ALL_DISC_2600.xlsx','ALL_DISC_2613.xlsx','ALL_DISC_26100.xlsx','random_2610.xlsx','ALL_DISC_2610.xls']:
   with self.assertRaises(InvalidData):period_from_filename(path,'DISC')
  with self.assertRaises(InvalidData):period_from_filename('ALL_FTG_2610.xlsx','DISC')
  for paths in [{'DISC':'ALL_DISC_2610.xlsx','FTG':'ALL_FTG_2611.xlsx'},{'DISC':'ALL_DISC_2610.xlsx','FTG':'ALL_FTG_2710.xlsx'},{'DISC':'ALL_DISC_2610.xlsx'}]:
   with self.assertRaises(InvalidData):period_from_inputs(paths)
 def test_period_controls_follow_file_selection(self):
  from app import App
  from types import SimpleNamespace
  class Variable:
   def __init__(self,value):self.value=value
   def get(self):return self.value
   def set(self,value):self.value=value
  app=SimpleNamespace(busy=False,auto_period=Variable(True),files={'DISC':Variable(''),'FTG':Variable('')},period_widgets=[],month=Variable(''),year=Variable(''),period_hint=Variable(''))
  App.sync_period(app);self.assertEqual(app.month.get(),'')
  app.files['DISC'].set('ALL_DISC_2610.xlsx');App.sync_period(app);self.assertEqual((app.month.get(),app.year.get()),('October','2026'))
  app.files['FTG'].set('ALL_FTG_2610.xlsx');App.sync_period(app);self.assertIn('both filenames agree',app.period_hint.get())
  app.files['FTG'].set('ALL_FTG_2611.xlsx');App.sync_period(app);self.assertEqual(app.month.get(),'');self.assertIn('different reporting periods',app.period_hint.get())
  app.auto_period.set(False);app.month.set('January');app.year.set('2027');App.sync_period(app);self.assertEqual((app.month.get(),app.year.get()),('January','2027'))
 def test_date_formats_and_period(self):
  for value,expected in [('10/13/2026 00:00:00',date(2026,10,13)),('13/10/2026',date(2026,10,13)),('2026-10-13',date(2026,10,13)),(datetime(2026,2,10),date(2026,10,2)),('02/10/2026',date(2026,10,2)),('10/02/2026',date(2026,10,2)),('13 October 2026',date(2026,10,13))]:
   self.assertEqual(document_date(value,10,2026)[0],expected)
 def test_reject_unresolvable_dates(self):
  for value in ['31/02/2026','2025-10-13','10/13/26','bad',None,True,'2026-10-13 ignored']:
   with self.assertRaises(ValueError):document_date(value,10,2026)
  with self.assertRaises(ValueError):document_date(datetime(2026,2,10),10,2026,False)
 def test_dates_other_month_and_serial(self):
  self.assertEqual(document_date('02/03/2026',2,2026)[0],date(2026,2,3))
  from openpyxl.utils.datetime import to_excel
  self.assertEqual(document_date(to_excel(datetime(2026,10,13)),10,2026)[0],date(2026,10,13))
 def test_numbers(self):
  self.assertEqual(number('(1,234.50)'),Decimal('-1234.50'))
  self.assertEqual(number(' 0.125 '),Decimal('.125'))
  self.assertEqual(whole('160.0'),160)
  for value in ['1,23',None,'NaN',True]:
   with self.assertRaises(ValueError):number(value)
  with self.assertRaises(ValueError):whole('160.5')
 def test_ids(self):
  self.assertEqual(identifier('001234'),'001234');self.assertEqual(identifier(160.0),'160');self.assertEqual(identifier('00123.0'),'00123')
 def test_name_mapping_duplicate_units_and_numeric_text(self):
  settings={'branches':{'C20':['KOTA KINABALU','KK']},'agencies':{'160':'KAO'},'column_aliases':{'date':['Transaction date']}}
  headers=['Branch Name','item No','Product Name','Transaction date','Document No','Customer Account','Customer Name','promotion_code','IPC Description','Invoice Qty','Unit','gross_amount','discount_amount','FOC Qty','Unit','FOC Amount','PGC','invoicemonth','invoiceyear']
  row=['KOTA KINABALU','00123','Product','10/13/2026 00:00:00','0009','0011','Customer','IPCF160-X','Offer','1.25','CS','1,000.50','10.25','0.5','EA','(1.50)','KAO','10.0','2026.0']
  with tempfile.TemporaryDirectory() as temp:
   path=Path(temp)/'raw.xlsx';w=Workbook();w.active.append(headers);w.active.append(row);w.save(path)
   b=Batch();read_input(path,'FTG',Options(10,2026),settings,b)
   self.assertEqual(b.errors,0,b.issues);r=b.rows['FTG'][0]
   self.assertEqual((r['branch'],r['pgc'],r['foc_unit']),('C20',160,'EA'));self.assertEqual(r['qty'],Decimal('1.25'));self.assertEqual(r['item'],'00123')
   chosen=selected(b.rows['FTG'],'FTG',False,Options(10,2026),settings)[0]
   self.assertEqual(chosen['foc_amount'],Decimal('1.50'));self.assertEqual(chosen['discount2'],Decimal('9.02'))
 def test_filters_signs_and_rounding(self):
  settings={'branches':{'C20':['KOTA KINABALU','KK']}}
  raw={'promotion':'IPCD160-X','pgc':160,'branch':'C20','discount':Decimal('-1.25'),'gross':Decimal('10'),'qty':Decimal('2'),'item':'0001'}
  r=selected([raw],'DISC',False,Options(10,2026),settings)[0]
  self.assertEqual(r['discount'],Decimal('1.25'));self.assertEqual(r['discount2'],Decimal('1.10'))
  self.assertEqual(selected([raw],'DISC',True,Options(10,2026),settings),[])
 def test_cancel(self):
  c=threading.Event();c.set()
  with self.assertRaises(Cancelled):check_cancel(c)
 def test_item_fallback_is_scoped_to_report_filter(self):
  settings={'branches':{'C20':['KOTA KINABALU','KK']}}
  common={'pgc':101,'branch':'C20','discount':Decimal('1'),'gross':Decimal('10'),'qty':Decimal('1'),'item_id':'fallback'}
  rows=[dict(common,promotion='IPCD-X',item='keep'),dict(common,promotion='IPCF-X',item='')]
  opt=Options(10,2026)
  self.assertEqual(selected(rows,'DISC',False,opt,settings)[0]['item'],'keep')
  self.assertEqual(selected(rows,'DISC',True,opt,settings)[0]['item'],'fallback')
 def test_mapping_persistence_and_validation(self):
  import json
  with tempfile.TemporaryDirectory() as temp:
   p=Path(temp)/'settings.json';p.write_text(json.dumps({'branches':{'C20':['KOTA KINABALU','KK']},'agencies':{'160':'KAO'},'company':'Test','column_aliases':{}}))
   save_agencies(p,{'160':'Updated KAO','999':'New agency'})
   data=load_settings(p);self.assertEqual(data['agencies'],{'160':'Updated KAO','999':'New agency'});self.assertEqual(data['company'],'Test')
   save_agencies(p,{'999':'New agency'});self.assertNotIn('160',load_settings(p)['agencies'])
   before=p.read_bytes()
   for mapping in [{'0':'bad'},{'1.5':'bad'},{'001':'bad'},{'160':''}]:
    with self.assertRaises(InvalidData):save_agencies(p,mapping)
    self.assertEqual(p.read_bytes(),before)
 def test_legacy_style_and_compact_header(self):
  settings={'branches':{'C20':['KOTA KINABALU','KK']},'agencies':{'160':'Edited agency'},'company':'Test'}
  raw={'item':'00123','product':'Product','date':date(2026,10,13),'document':'DOC','customer':'001','customer_name':'Customer','promotion':'IPCD-X','description':'Offer','pgc':160,'branch':'C20','discount':Decimal('1.25'),'gross':Decimal('10'),'qty':Decimal('2'),'unit':'EA'}
  opt=Options(10,2026);row=selected([raw],'DISC',False,opt,settings)[0]
  with tempfile.TemporaryDirectory() as temp:
   p=Path(temp)/'test.xlsx';w=Workbook(write_only=True);write_sheet(w,'P160',[row],'DISC',opt,settings,'KOTA KINABALU',160);w.save(p);check=load_workbook(p);s=check.active
   self.assertEqual(s['A3'].value,'Branch : KOTA KINABALU');self.assertEqual(s['A5'].value,'Agency : Edited agency');self.assertIn('A3:B3',str(s.merged_cells));self.assertEqual(s['A7'].value,'ITEM NO');self.assertEqual(s['O7'].value,'BRANCH');self.assertIsNone(s['N7'].value)
   self.assertEqual(s['A7'].fill.fgColor.rgb,'00FFDAB9');self.assertEqual(s['O7'].fill.fgColor.rgb,'0092D050')
   self.assertEqual(s['L9'].border.top.color.rgb,'FF808080');self.assertEqual(s['C8'].number_format,'d/m/yyyy');check.close()

if __name__=='__main__':unittest.main()
