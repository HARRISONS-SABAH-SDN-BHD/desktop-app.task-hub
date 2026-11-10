from __future__ import annotations
import calendar, json, os, queue, sys, threading, traceback
from time import perf_counter
from datetime import datetime
from decimal import Decimal
from pathlib import Path
import tkinter as tk
from tkinter import ttk, filedialog, messagebox
from engine import Options, Batch, read_input, generate, load_settings, write_issues, Cancelled, InvalidData, period_from_inputs, selected_input_paths
from ig_editor import IGEditor
from main_view import build_ui
from menu_view import build_menu
from run_timing import timing_record,timing_label
from completion_sound import play_completion_sound

BASE=Path(sys.executable).parent if getattr(sys,'frozen',False) else Path(__file__).resolve().parent.parent
VERSION='1.17.0'
ASSETS=Path(getattr(sys,'_MEIPASS',BASE))/'assets'

class App(tk.Tk):
    def __init__(self):
        if sys.platform=='win32':
            import ctypes
            ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID('Harrison.ExcelFlow.Desktop')
        super().__init__()
        self.iconbitmap(default=str(ASSETS/'app.ico'))
        self._app_icon_images=[tk.PhotoImage(file=str(ASSETS/f'app-logo-{size}.png')) for size in [16,32,48,64]]
        self.iconphoto(True,*self._app_icon_images)
        build_ui(self)
        build_menu(self)
        self.show_main_menu()

    def show_main_menu(self):
        if self.busy:return
        self.details_frame.pack_forget();self.report_frame.pack_forget();self.menu_frame.pack(fill='both',expand=True)
        self.current_screen='menu';self.title('Harrison ExcelFlow — Main menu')

    def show_report(self):
        self.details_frame.pack_forget();self.menu_frame.pack_forget();self.report_frame.pack(fill='both',expand=True)
        self.current_screen='report';self.title('Harrison ExcelFlow — DISC & FTG Reports')

    def show_run_details(self):
        self.report_frame.pack_forget();self.menu_frame.pack_forget()
        self.details_frame.pack(fill='both',expand=True)
        self.current_screen='details';self.title('Harrison ExcelFlow — Run details')

    def set_progress(self,percent):
        value=max(0,min(100,int(round(percent))))
        self.progress_value=max(self.progress_value,value)
        self.progress.stop();self.progress.configure(mode='determinate',value=self.progress_value)
        self.progress_text.set(f'{self.progress_value}%')

    def notify_completed(self):
        if getattr(self,'_completion_sound_played',False):return
        self._completion_sound_played=True
        play_completion_sound(ASSETS/'completion.wav')

    def show_timings(self,timings):
        label=timing_label(timings)
        self.timing_text.set(label);self.detail_label.grid_remove();self.timing_label.grid()
        self.append_log(label)

    def sync_period(self):
        if self.busy:return
        automatic=self.auto_period.get()
        if hasattr(self,'period_caption'):self.period_caption.set('Detected automatically from filenames' if automatic else 'Manual reporting period')
        for c in self.period_widgets:c.configure(state='disabled' if automatic else 'readonly' if isinstance(c,ttk.Combobox) else 'normal')
        if not automatic:
            if not self.month.get():self.month.set(calendar.month_name[datetime.now().month])
            if not self.year.get():self.year.set(str(datetime.now().year))
            self.period_hint.set('Manual period enabled. Select the month and year that match the raw data.');return
        try:
            scope=self.report_scope.get() if hasattr(self,'report_scope') else 'Both'
            paths=selected_input_paths({k:v.get() for k,v in self.files.items()},scope,False)
            detected=period_from_inputs(paths,require_both=False)
            if detected:
                month,year=detected;self.month.set(calendar.month_name[month]);self.year.set(str(year))
                self.period_hint.set(f'Detected: {calendar.month_name[month]} {year}'+(' — both filenames agree.' if len(paths)==2 else f' — {next(iter(paths))} input ready.'))
            else:
                self.month.set('');self.year.set('');self.period_hint.set('Select filenames such as ALL_DISC_2610.xlsx — the suffix is YYMM.')
        except InvalidData as e:
            self.month.set('');self.year.set('');self.period_hint.set(str(e))

    def browse(self,kind):
        p=filedialog.askopenfilename(title=f'Select raw {kind} workbook',filetypes=[('Excel workbooks','*.xlsx *.xlsm')])
        if p:self.files[kind].set(p)
    def browse_destination(self):
        p=filedialog.askdirectory(title='Choose output folder')
        if p:self.destination.set(p)
    def edit_settings(self):
        try:IGEditor(self,BASE/'settings.json')
        except Exception as e:messagebox.showerror('Could not open IG mappings',str(e))
    def append_log(self,msg):
        self.log.configure(state='normal');self.log.insert('end',msg+'\n');self.log.see('end');self.log.configure(state='disabled')
    def open_output(self):
        if self.last_output and self.last_output.exists():os.startfile(self.last_output)
    def start(self,build):
        if self.busy:return
        try:
            paths={k:Path(v) for k,v in selected_input_paths({k:v.get().strip() for k,v in self.files.items()},self.report_scope.get()).items()}
            for k,p in paths.items():
                if not p.is_file() or p.suffix.lower() not in ['.xlsx','.xlsm']:raise ValueError(f'{k}: select an existing .xlsx or .xlsm workbook.')
            if len(paths)==2 and paths['DISC'].resolve()==paths['FTG'].resolve():raise ValueError('DISC and FTG must be different files.')
            if self.auto_period.get():
                month,year=period_from_inputs(paths,require_both=False);self.month.set(calendar.month_name[month]);self.year.set(str(year))
            year=int(self.year.get());money=int(self.money.get());qty=int(self.quantity.get());factor=Decimal(self.factor.get())
            if not 1900<=year<=2200 or not 0<=money<=6 or not 0<=qty<=8 or not factor.is_finite() or not Decimal('0')<=factor<=Decimal('10'):raise ValueError('Check year, decimal places (0–6 / 0–8), and multiplier (0–10).')
            opt=Options(list(calendar.month_name).index(self.month.get()),year,self.mode.get(),money,qty,self.repair.get(),factor);settings=load_settings(BASE/'settings.json')
            if not self.destination.get().strip():raise ValueError('Choose an output folder.')
            dest=Path(self.destination.get().strip());dest.mkdir(parents=True,exist_ok=True)
        except Exception as e:messagebox.showerror('Check your selections',str(e));return
        self._completion_sound_played=False
        self.busy=True;self.last_output=None;self.cancel.clear();self.progress_value=0;self.set_progress(0);self.cancel_button.configure(state='normal');self.open_button.configure(state='disabled')
        self.timing_text.set('');self.timing_label.grid_remove();self.detail_label.grid()
        for c in self.controls:c.configure(state='disabled')
        for tree in [self.tree,self.issues]:tree.delete(*tree.get_children())
        if hasattr(self,'show_preview_placeholder'):self.show_preview_placeholder(True)
        self.append_log(f'\n{calendar.month_name[opt.month]} {opt.year} • {opt.mode} • '+', '.join(paths)+' • '+('Generate' if build else 'Validate'));self.status.set('Reading and validating your workbook'+('s…' if len(paths)>1 else '…'));self.detail.set('Validation checks every row before any reports are generated.')
        threading.Thread(target=self.work,args=(paths,opt,settings,dest,build),daemon=True).start()
    def work(self,paths,opt,settings,dest,build):
        try:
            started=perf_counter()
            batch=Batch()
            read_share=30 if build else 95
            for index,(kind,path) in enumerate(paths.items()):
                def read_progress(message,percent,index=index):
                    overall=None if percent is None else read_share*(index+percent/100)/len(paths)
                    self.q.put(('progress',message,overall))
                read_progress(f'Reading {kind} workbook…',0)
                read_input(path,kind,opt,settings,batch,read_progress,self.cancel)
                read_progress(f'{kind} validation finished',100)
            validation_finished=perf_counter();validation_seconds=validation_finished-started
            self.q.put(('validated',batch))
            if batch.errors:
                folder=dest/('Validation '+datetime.now().strftime('%Y%m%d_%H%M%S_%f'));folder.mkdir();write_issues(folder/'validation_issues.csv',batch);self.q.put(('invalid',folder,batch.errors,timing_record(validation_seconds)));return
            if build:
                def progress(message,percent):self.q.put(('progress',message,None if percent is None else min(99,30+.7*percent)))
                result,summary=generate(batch,opt,settings,dest,progress,self.cancel)
                summary['timings']=timing_record(validation_seconds,perf_counter()-validation_finished)
                summary['source_files']={k:str(p.resolve()) for k,p in paths.items()};(result/'run_summary.json').write_text(json.dumps(summary,indent=2),encoding='utf-8');self.q.put(('complete',result,summary))
            else:self.q.put(('ready',timing_record(validation_seconds)))
        except Cancelled as e:self.q.put(('cancelled',str(e),getattr(e,'output_dir',None)))
        except Exception as e:self.q.put(('error',str(e),traceback.format_exc(),getattr(e,'output_dir',None)))
        finally:self.q.put(('idle',))
    def poll(self):
        try:
            while True:
                item=self.q.get_nowait();event=item[0]
                if event=='progress':
                    self.status.set(item[1])
                    if item[2] is not None:self.set_progress(item[2])
                elif event=='validated':
                    batch=item[1]
                    for row in batch.preview:self.tree.insert('','end',values=row)
                    if hasattr(self,'show_preview_placeholder'):self.show_preview_placeholder(not bool(batch.preview))
                    for issue in batch.issues[:500]:self.issues.insert('','end',values=issue[1:])
                    for k,st in batch.stats.items():self.append_log(f"{k}: {st['source_rows']:,} rows; {st['period_date_corrections']:,} dates resolved using period; {st['numeric_text_converted']:,} numeric text values converted.")
                    warnings=len(batch.issues)-batch.errors
                    self.append_log(f'Validation errors: {batch.errors:,}; warnings: {warnings:,}'+(' (first 500 issues shown; all included in CSV)' if len(batch.issues)>500 else ''))
                elif event=='invalid':
                    self.last_output=item[1];self.status.set(f'{item[2]:,} validation errors. Reports were not generated.');self.detail.set('Click View run details → Issues, or open the complete validation_issues.csv in Open results.');self.append_log(str(item[1]))
                    if len(item)>3:self.show_timings(item[3])
                elif event=='ready':
                    self.status.set('Validation passed. Ready to generate reports.');self.detail.set('Click View run details to review dates, then return to generate reports.');self.set_progress(100)
                    if len(item)>1:self.show_timings(item[1])
                elif event=='complete':
                    self.set_progress(100)
                    self.last_output=item[1];summary=item[2];self.status.set(f"Complete — {len(summary['files'])} workbooks generated.");self.detail.set(str(item[1]));self.append_log('Saved: '+str(item[1]))
                    if summary.get('timings'):self.show_timings(summary['timings'])
                    for empty in summary['empty_reports']:self.append_log(f"{empty['mode']} {empty['kind']}: no matching promotion codes; no workbook required.")
                    self.notify_completed()
                elif event=='cancelled':self.last_output=item[2];self.status.set(item[1]);self.detail.set('Any partial run is marked cancelled in its run_summary.json. Completed files in that run may be incomplete as a set.')
                elif event=='error':
                    self.last_output=item[3];self.status.set('Processing stopped.');self.detail.set(item[1]);self.append_log(item[2]);messagebox.showerror('Processing stopped',item[1])
                elif event=='idle':
                    self.busy=False;self.progress.stop();self.cancel_button.configure(state='disabled')
                    self.progress.configure(value=self.progress_value)
                    for c in self.controls:c.configure(state='readonly' if isinstance(c,ttk.Combobox) else 'normal')
                    self.sync_period()
                    self.open_button.configure(state='normal' if self.last_output else 'disabled')
        except queue.Empty:pass
        self.after(100,self.poll)
    def close(self):
        if self.busy:
            self.cancel.set();self.status.set('Cancelling… close the window again when processing stops.');return
        self.destroy()

if __name__=='__main__':App().mainloop()
