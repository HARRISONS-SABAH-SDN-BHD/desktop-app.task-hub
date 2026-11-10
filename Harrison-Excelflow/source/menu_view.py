"""Main menu; report widgets stay mounted to preserve the current session."""
from tkinter import ttk
from ui_widgets import Card,Symbol,IconButton,BG,TEAL

def build_menu(app):
    menu=ttk.Frame(app,padding=(36,30));app.menu_frame=menu
    ttk.Label(menu,text='Harrison ExcelFlow',style='Title.TLabel').pack(anchor='w')
    ttk.Label(menu,text='Choose a module to get started.',style='Muted.TLabel',font=('Segoe UI',12)).pack(anchor='w',pady=(8,28))
    card=Card(menu,padding=28);card.pack(anchor='nw',fill='x')
    body=card.content;body.columnconfigure(1,weight=1)
    Symbol(body,kind='sheet',size=52).grid(row=0,column=0,rowspan=2,sticky='n',padx=(0,22))
    ttk.Label(body,text='DISC & FTG Reports',style='Section.TLabel',font=('Segoe UI',19,'bold')).grid(row=0,column=1,sticky='w')
    ttk.Label(body,text='Create monthly reports from a DISC workbook, an FTG workbook, or both.\nIncludes date validation, IG name mappings and your saved report format.',style='Hint.TLabel',font=('Segoe UI',11),wraplength=900).grid(row=1,column=1,sticky='w',pady=(10,24))
    IconButton(body,text='DISC & FTG Reports',kind='play',style='Accent.TButton',command=app.show_report,width=285).grid(row=2,column=1,sticky='w',pady=(0,6))
    upcoming=ttk.Frame(menu,padding=(0,24));upcoming.pack(fill='x')
    ttk.Label(upcoming,text='More to come',style='Muted.TLabel',font=('Segoe UI',14,'bold'),anchor='center').pack(fill='x')
    message=ttk.Label(upcoming,text='New modules and features will be added in future updates.',style='Muted.TLabel',font=('Segoe UI',11),anchor='center',justify='center',wraplength=850)
    message.pack(fill='x',pady=(7,0))
    upcoming.bind('<Configure>',lambda event:message.configure(wraplength=max(260,event.width-40)))
    ttk.Label(menu,text='Harrison ExcelFlow  •  Excel automation & reporting',style='Muted.TLabel').pack(side='bottom',anchor='w',pady=(24,0))
