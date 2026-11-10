"""Spacious desktop layout; report logic lives in engine.py."""
import calendar,queue,threading
from datetime import datetime
from pathlib import Path
import tkinter as tk
from tkinter import ttk
from ui_widgets import Card,IconButton,Symbol,section,ChoiceGroup
from default_paths import desktop_directory

BG='#EAF0F6';WHITE='#FFFFFF';INK='#12263D';MUTED='#526780';ACCENT='#006A70'

def build_ui(app):
    app.title('Harrison ExcelFlow — DISC & FTG Reports')
    width=min(1460,app.winfo_screenwidth()-60);height=min(800,app.winfo_screenheight()-90)
    app.geometry(f'{width}x{height}');app.minsize(980,750);app.configure(bg=BG)
    app.q=queue.Queue();app.cancel=threading.Event();app.busy=False;app.last_output=None;app.progress_text=tk.StringVar(value='0%');app.progress_value=0;app.timing_text=tk.StringVar(value='')
    app.files={k:tk.StringVar() for k in ['DISC','FTG']};app.destination=tk.StringVar(value=str(desktop_directory()))
    app.month=tk.StringVar();app.year=tk.StringVar();app.mode=tk.StringVar(value='Both');app.auto_period=tk.BooleanVar(value=True)
    app.report_scope=tk.StringVar(value='Both')
    app.period_hint=tk.StringVar(value='The month and year will be detected from your filenames.')
    app.period_caption=tk.StringVar(value='Detected automatically from filenames')
    app.money=tk.StringVar(value='2');app.quantity=tk.StringVar(value='2');app.factor=tk.StringVar(value='0.88');app.repair=tk.BooleanVar(value=True)
    app.status=tk.StringVar(value='Ready when you are');app.detail=tk.StringVar(value='Choose a DISC workbook, an FTG workbook, or both to get started.')
    app.controls=[];app.period_widgets=[]
    style=ttk.Style(app);style.theme_use('clam')
    style.configure('.',font=('Segoe UI',11))
    style.configure('TFrame',background=BG);style.configure('Card.TFrame',background=WHITE)
    style.configure('TLabel',background=BG,foreground=INK)
    style.configure('Card.TLabel',background=WHITE,foreground=INK)
    style.configure('Title.TLabel',font=('Segoe UI',25,'bold'),foreground=INK,background=BG)
    style.configure('Section.TLabel',font=('Segoe UI',14,'bold'),background=WHITE,foreground=INK)
    style.configure('Field.TLabel',font=('Segoe UI',11,'bold'),background=WHITE,foreground=INK)
    style.configure('Hint.TLabel',font=('Segoe UI',10),foreground=MUTED,background=WHITE)
    style.configure('Muted.TLabel',font=('Segoe UI',10),foreground=MUTED,background=BG)
    style.configure('Status.TLabel',font=('Segoe UI',12,'bold'),background=WHITE,foreground=INK)
    style.configure('TEntry',padding=(10,9),fieldbackground='#F8FAFC',bordercolor='#D8E1E9',lightcolor='#D8E1E9',darkcolor='#D8E1E9')
    style.configure('TCombobox',padding=(10,8),fieldbackground='#F8FAFC',bordercolor='#D8E1E9',arrowsize=14)
    style.configure('TSpinbox',padding=(10,8),fieldbackground='#F8FAFC',bordercolor='#D8E1E9',arrowsize=12)
    style.configure('Period.TCombobox',foreground=INK)
    style.map('Period.TCombobox',foreground=[('disabled','#999999'),('readonly',INK)])
    style.map('TSpinbox',foreground=[('disabled','#999999')])
    style.configure('TButton',font=('Segoe UI',11),padding=(16,10),background='#E7EDF3',foreground=INK,borderwidth=0)
    style.map('TButton',background=[('active','#D9E4EE'),('disabled','#EDF1F5')],foreground=[('disabled','#9CAAB6')])
    style.configure('Accent.TButton',background=ACCENT,foreground=WHITE,font=('Segoe UI',11,'bold'),padding=(22,11))
    style.map('Accent.TButton',background=[('active','#0E5367'),('disabled','#A7BBC4')],foreground=[('disabled',WHITE)])
    style.configure('TCheckbutton',background=WHITE,foreground=INK,padding=(0,6))
    style.configure('TNotebook',background=WHITE,borderwidth=0,tabmargins=(8,4,8,0))
    style.configure('TNotebook.Tab',padding=(18,11),background='#EFF3F7',foreground=MUTED)
    style.map('TNotebook.Tab',background=[('selected',WHITE)],foreground=[('selected',INK)],padding=[('selected',(18,11)),('!selected',(18,11))],expand=[('selected',(0,0,0,0))])
    style.configure('Segment.Toolbutton',font=('Segoe UI',12),padding=(18,9),anchor='center',background='#F7FAFD',foreground=INK,relief='flat',borderwidth=1)
    style.map('Segment.Toolbutton',background=[('selected',ACCENT),('active','#E4EEF3')],foreground=[('disabled','#9AA9B6'),('selected',WHITE)])
    style.configure('Flat.TEntry',padding=(6,8),borderwidth=0,relief='flat',fieldbackground='#F7FAFD')
    style.configure('Treeview',font=('Segoe UI',10),rowheight=34,background=WHITE,fieldbackground=WHITE,foreground=INK,borderwidth=0)
    style.configure('Treeview.Heading',font=('Segoe UI',10,'bold'),padding=(10,9),background='#DFE7EF',foreground=INK,relief='flat')
    style.configure('Horizontal.TProgressbar',background=ACCENT,troughcolor='#E8EEF4',borderwidth=0)

    outer=ttk.Frame(app,padding=(28,22));outer.pack(fill='both',expand=True);outer.columnconfigure(0,weight=1);app.report_frame=outer
    header=ttk.Frame(outer);header.grid(row=0,column=0,sticky='ew',pady=(0,22));header.columnconfigure(0,weight=1)
    ttk.Label(header,text='DISC & FTG Reports',style='Title.TLabel').grid(row=0,column=0,sticky='w')
    ttk.Label(header,text='Generate your DISC & FTG monthly reports in a few simple steps.',style='Muted.TLabel',font=('Segoe UI',11)).grid(row=1,column=0,sticky='w',pady=(5,0))
    tools=ttk.Frame(header);tools.grid(row=0,column=1,rowspan=2,sticky='e')
    for label,command,kind in [('Main menu',app.show_main_menu,'home'),('Manage IG names',app.edit_settings,'users'),('Settings',lambda:open_preferences(app),'settings')]:
        b=IconButton(tools,text=label,command=command,kind=kind);b.pack(side='left',padx=(12,0));app.controls.append(b)

    upper=ttk.Frame(outer);upper.grid(row=1,column=0,sticky='ew',pady=(0,12));upper.columnconfigure(0,weight=3);upper.columnconfigure(1,weight=2,minsize=390)
    files_card=Card(upper,padding=20);files_card.grid(row=0,column=0,sticky='nsew',padx=(0,12));files=files_card.content;files.columnconfigure(0,weight=1)
    section(files,'01','Workbooks')
    def input_field(parent,var,kind='excel',placeholder=''):
        frame=tk.Frame(parent,bg='#F7FAFD',highlightthickness=1,highlightbackground='#BCCBD8');Symbol(frame,kind=kind,size=23,bg='#F7FAFD').pack(side='left',padx=(9,2))
        entry=ttk.Entry(frame,textvariable=var,style='Flat.TEntry');entry.pack(side='left',fill='both',expand=True)
        if placeholder:
            label=tk.Label(entry,text=placeholder,bg='#F7FAFD',fg=MUTED,font=('Segoe UI',11),anchor='w',cursor='xterm')
            def refresh(*_):
                if not var.get() and entry.focus_get()!=entry:label.place(x=7,y=8)
                else:label.place_forget()
            label.bind('<Button-1>',lambda _:entry.focus_set());entry.bind('<FocusIn>',refresh);entry.bind('<FocusOut>',refresh);var.trace_add('write',refresh);refresh()
        return frame,entry
    for i,kind in enumerate(['DISC','FTG']):
        row=1+i*2
        ttk.Label(files,text=f'{kind} workbook',style='Field.TLabel').grid(row=row,column=0,columnspan=2,sticky='w',pady=(0,7))
        frame,e=input_field(files,app.files[kind],placeholder='Choose an .xlsx file');frame.grid(row=row+1,column=0,sticky='ew',pady=(0,14));app.controls.append(e)
        b=IconButton(files,text='Browse…',kind='folder',command=lambda k=kind:app.browse(k));b.grid(row=row+1,column=1,sticky='e',padx=(12,0),pady=(0,14));app.controls.append(b)
    hint=ttk.Label(files,textvariable=app.period_hint,style='Hint.TLabel',wraplength=600)
    hint.grid(row=5,column=0,columnspan=2,sticky='w')
    files.bind('<Configure>',lambda e:hint.configure(wraplength=max(260,e.width-40)))

    period_card=Card(upper,padding=20);period_card.grid(row=0,column=1,sticky='nsew');period=period_card.content;period.columnconfigure(0,weight=1);period.columnconfigure(1,weight=1)
    section(period,'02','Report details')
    ttk.Label(period,text='Month',style='Hint.TLabel').grid(row=1,column=0,sticky='w',pady=(0,6))
    ttk.Label(period,text='Year',style='Hint.TLabel').grid(row=1,column=1,sticky='w',padx=(12,0),pady=(0,6))
    cb=ttk.Combobox(period,textvariable=app.month,values=list(calendar.month_name)[1:],style='Period.TCombobox',state='disabled',width=13)
    cb.grid(row=2,column=0,sticky='ew');app.controls.append(cb);app.period_widgets.append(cb)
    sp=ttk.Spinbox(period,textvariable=app.year,from_=2000,to=2100,state='disabled',width=6)
    sp.grid(row=2,column=1,sticky='ew',padx=(12,0));app.controls.append(sp);app.period_widgets.append(sp)
    ttk.Label(period,textvariable=app.period_caption,style='Hint.TLabel',wraplength=280).grid(row=3,column=0,columnspan=2,sticky='w',pady=(9,16))
    ttk.Label(period,text='Workbooks to process',style='Field.TLabel').grid(row=4,column=0,columnspan=2,sticky='w',pady=(0,7))
    app.workbook_choices=ChoiceGroup(period,app.report_scope,['Both','DISC','FTG']);app.workbook_choices.grid(row=5,column=0,columnspan=2,sticky='ew');app.controls.extend(app.workbook_choices.buttons)
    ttk.Label(period,text='Report version',style='Field.TLabel').grid(row=6,column=0,columnspan=2,sticky='w',pady=(12,7))
    app.version_choices=ChoiceGroup(period,app.mode,['Both','Normal','Special']);app.version_choices.grid(row=7,column=0,columnspan=2,sticky='ew');app.controls.extend(app.version_choices.buttons)

    dest_card=Card(outer,padding=18);dest_card.grid(row=2,column=0,sticky='ew',pady=(0,16));dest=dest_card.content;dest.columnconfigure(1,weight=1)
    section(dest,'03','Output location',span=3)
    ttk.Label(dest,text='Save reports to',style='Card.TLabel').grid(row=1,column=0,sticky='w',padx=(0,16))
    frame,e=input_field(dest,app.destination,kind='folder');frame.grid(row=1,column=1,sticky='ew');app.controls.append(e)
    b=IconButton(dest,text='Choose folder…',kind='folder',command=app.browse_destination);b.grid(row=1,column=2,padx=(16,0));app.controls.append(b)

    actions=ttk.Frame(outer);actions.grid(row=3,column=0,sticky='ew',pady=(0,18))
    app.generate_button=IconButton(actions,text='Generate reports',kind='play',style='Accent.TButton',command=lambda:app.start(True));app.generate_button.pack(side='left');app.controls.append(app.generate_button)
    app.validate_button=IconButton(actions,text='Validate & preview',kind='preview',command=lambda:app.start(False));app.validate_button.pack(side='left',padx=(14,0));app.controls.append(app.validate_button)
    app.cancel_button=IconButton(actions,text='Cancel',command=app.cancel.set,state='disabled',width=125);app.cancel_button.pack(side='left',padx=(14,0))
    app.open_button=IconButton(actions,text='Open results',kind='open',command=app.open_output,state='disabled');app.open_button.pack(side='right')

    app.details_button=IconButton(actions,text='View run details',kind='preview',command=app.show_run_details)
    app.details_button.pack(side='left',padx=(14,0))

    details=ttk.Frame(app,padding=(28,22));app.details_frame=details
    details.columnconfigure(0,weight=1);details.rowconfigure(1,weight=1)
    heading=ttk.Frame(details);heading.grid(row=0,column=0,sticky='ew',pady=(0,22));heading.columnconfigure(0,weight=1)
    ttk.Label(heading,text='Run details',style='Title.TLabel').grid(row=0,column=0,sticky='w')
    ttk.Label(heading,text='Review dates, follow processing, and check any issues.',style='Muted.TLabel').grid(row=1,column=0,sticky='w',pady=(5,0))
    IconButton(heading,text='Back to report',command=app.show_report).grid(row=0,column=1,rowspan=2,sticky='e')
    results_card=Card(details,padding=16);results_card.grid(row=1,column=0,sticky='nsew',pady=(0,16));results=results_card.content;results.rowconfigure(0,weight=1);results.columnconfigure(0,weight=1)
    tabs=ttk.Notebook(results);tabs.grid(row=0,column=0,sticky='nsew');app.details_tabs=tabs
    def table_tab(label,columns):
        frame=ttk.Frame(tabs,style='Card.TFrame',padding=(10,10));tabs.add(frame,text=label);frame.rowconfigure(0,weight=1);frame.columnconfigure(0,weight=1)
        tree=ttk.Treeview(frame,columns=[c[0] for c in columns],show='headings',height=4)
        for key,title,width,stretch in columns:tree.heading(key,text=title);tree.column(key,width=width,minwidth=50,stretch=stretch)
        tree.grid(row=0,column=0,sticky='nsew');vs=ttk.Scrollbar(frame,orient='vertical',command=tree.yview);vs.grid(row=0,column=1,sticky='ns')
        hs=ttk.Scrollbar(frame,orient='horizontal',command=tree.xview);hs.grid(row=1,column=0,sticky='ew');tree.configure(yscrollcommand=vs.set,xscrollcommand=hs.set)
        return tree
    app.tree=table_tab('Date preview',[('input','Input',65,False),('row','Excel row',85,False),('raw','Original value',270,True),('date','Normalized date',160,False),('action','Result',240,True)])
    placeholder=ttk.Frame(app.tree,style='Card.TFrame')
    empty_icon=Symbol(placeholder,kind='sheet',size=48);empty_icon.pack(pady=(0,6))
    empty_title=ttk.Label(placeholder,text='Your preview will appear here',style='Section.TLabel',font=('Segoe UI',12,'bold'));empty_title.pack()
    ttk.Label(placeholder,text='Add one or both workbooks, then validate to check dates.',style='Hint.TLabel').pack(pady=(5,0))
    placeholder_state={'visible':True}
    def show_placeholder(show):
        placeholder_state['visible']=show
        if show:
            body_height=max(40,app.tree.winfo_height()-56)
            if body_height<115:empty_icon.pack_forget()
            elif not empty_icon.winfo_manager():empty_icon.pack(before=empty_title,pady=(0,6))
            placeholder.place(x=max(1,app.tree.winfo_width())/2,y=40+body_height/2,anchor='center')
        else:placeholder.place_forget()
    app.tree.bind('<Configure>',lambda _:show_placeholder(placeholder_state['visible']))
    app.show_preview_placeholder=show_placeholder;show_placeholder(True)
    logframe=ttk.Frame(tabs,style='Card.TFrame',padding=16);tabs.add(logframe,text='Run log');logframe.rowconfigure(0,weight=1);logframe.columnconfigure(0,weight=1)
    app.log=tk.Text(logframe,font=('Consolas',10),wrap='word',height=4,bg=WHITE,fg=INK,relief='flat',state='disabled',padx=4,pady=4)
    app.log.grid(row=0,column=0,sticky='nsew');scroll=ttk.Scrollbar(logframe,orient='vertical',command=app.log.yview);scroll.grid(row=0,column=1,sticky='ns');app.log.configure(yscrollcommand=scroll.set)
    app.issues=table_tab('Issues',[('kind','Input',65,False),('row','Row',70,False),('column','Column',150,False),('value','Raw value',190,True),('message','Issue',500,True)])

    details_status=Card(details,padding=16);details_status.grid(row=2,column=0,sticky='ew')
    footer=details_status.content;footer.columnconfigure(0,weight=1)
    ttk.Label(footer,textvariable=app.status,style='Status.TLabel',wraplength=850).grid(row=0,column=0,sticky='w')
    ttk.Label(footer,textvariable=app.progress_text,style='Status.TLabel').grid(row=0,column=1,sticky='e',padx=(20,0))
    ttk.Label(footer,textvariable=app.timing_text,style='Hint.TLabel',wraplength=850).grid(row=1,column=0,sticky='w',pady=(5,0))

    status_card=Card(outer,padding=13);status_card.grid(row=4,column=0,sticky='ew');status=status_card.content;status.columnconfigure(1,weight=1)
    ttk.Label(status,textvariable=app.status,style='Status.TLabel',wraplength=340).grid(row=0,column=0,sticky='w',padx=(0,24))
    detail=ttk.Label(status,textvariable=app.detail,style='Hint.TLabel',wraplength=760)
    app.detail_label=detail
    detail.grid(row=0,column=1,sticky='w');status.bind('<Configure>',lambda e:detail.configure(wraplength=max(220,e.width-540)))
    app.progress=ttk.Progressbar(status,maximum=100,length=150);app.progress.grid(row=0,column=2,sticky='e',padx=(20,8))
    ttk.Label(status,textvariable=app.progress_text,style='Status.TLabel',width=5,anchor='e').grid(row=0,column=3,sticky='e')
    app.timing_label=ttk.Label(status,textvariable=app.timing_text,style='Hint.TLabel',font=('Segoe UI',10,'bold'),foreground=ACCENT,wraplength=740)
    app.timing_label.grid(row=0,column=1,sticky='w');app.timing_label.grid_remove()
    status.bind('<Configure>',lambda e:app.timing_label.configure(wraplength=max(220,e.width-540)),add='+')
    app.protocol('WM_DELETE_WINDOW',app.close)
    for var in app.files.values():var.trace_add('write',lambda *_:app.sync_period())
    app.report_scope.trace_add('write',lambda *_:app.sync_period())
    app.sync_period();app.after(100,app.poll)

def open_preferences(app):
    if app.busy:return
    win=tk.Toplevel(app);win.title('Report settings');win.configure(bg=WHITE);win.geometry('630x660');win.resizable(False,False);win.transient(app);win.grab_set()
    body=ttk.Frame(win,style='Card.TFrame',padding=28);body.pack(fill='both',expand=True);body.columnconfigure(1,weight=1)
    ttk.Label(body,text='Report settings',style='Section.TLabel',font=('Segoe UI',19,'bold')).grid(row=0,column=0,columnspan=2,sticky='w',pady=(0,8))
    ttk.Label(body,text='Defaults are set to match your existing reports.',style='Hint.TLabel').grid(row=1,column=0,columnspan=2,sticky='w',pady=(0,22))
    for i,(label,var,lo,hi) in enumerate([('Amount (RM) decimals',app.money,0,6),('Quantity decimals',app.quantity,0,8)]):
        row=3+i*2;ttk.Label(body,text=label,style='Card.TLabel').grid(row=row,column=0,sticky='w',pady=(0,6));ttk.Spinbox(body,textvariable=var,from_=lo,to=hi,width=7).grid(row=row,column=1,sticky='e',pady=(0,6))
        hint='Displayed monetary precision; also used for P160 rounding.' if i==0 else 'Displayed precision; underlying quantities are retained.'
        ttk.Label(body,text=hint,style='Hint.TLabel',wraplength=520).grid(row=row+1,column=0,columnspan=2,sticky='w',pady=(0,16))
    ttk.Label(body,text='P160 multiplier',style='Card.TLabel').grid(row=7,column=0,sticky='w',pady=(0,16));ttk.Entry(body,textvariable=app.factor,width=9).grid(row=7,column=1,sticky='e',pady=(0,16))
    ttk.Separator(body).grid(row=8,column=0,columnspan=2,sticky='ew',pady=(0,12))
    ttk.Checkbutton(body,text='Detect month and year from filenames (YYMM)',variable=app.auto_period,command=app.sync_period).grid(row=9,column=0,columnspan=2,sticky='w')
    ttk.Checkbutton(body,text='Recover swapped dates using the reporting period',variable=app.repair).grid(row=10,column=0,columnspan=2,sticky='w')
    ttk.Label(body,text='For example, October 2026 + 10/13/2026 becomes 13/10/2026.\nTurn off filename detection to choose the month and year manually.',style='Hint.TLabel',wraplength=530).grid(row=11,column=0,columnspan=2,sticky='w',pady=(6,22))
    IconButton(body,text='Done',style='Accent.TButton',command=win.destroy).grid(row=12,column=1,sticky='e')
    win.update_idletasks();x=max(0,(app.winfo_screenwidth()-610)//2);y=max(0,(app.winfo_screenheight()-600)//2);win.geometry(f'+{x}+{y}')
