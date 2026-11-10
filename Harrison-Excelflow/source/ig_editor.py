"""In-app, searchable IG/agency mapping editor with explicit Save and Cancel."""
import re
import tkinter as tk
from tkinter import ttk, messagebox
from engine import load_settings, save_agencies
from ui_widgets import IconButton

class IGEditor(tk.Toplevel):
    def __init__(self,parent,settings_path):
        super().__init__(parent);self.title('IG numbers & agency names');self.geometry('790x650');self.minsize(650,520)
        self.path=settings_path;self.mapping=dict(load_settings(settings_path).get('agencies',{}));self.original=dict(self.mapping);self.selected=None
        self.query=tk.StringVar();self.code=tk.StringVar();self.name=tk.StringVar();self.count=tk.StringVar()
        self.transient(parent);self.grab_set();self.protocol('WM_DELETE_WINDOW',self.close)
        body=ttk.Frame(self,padding=18);body.pack(fill='both',expand=True);body.columnconfigure(0,weight=1);body.rowconfigure(2,weight=1)
        ttk.Label(body,text='IG numbers & agency names',font=('Segoe UI',17,'bold')).grid(row=0,column=0,sticky='w')
        search=ttk.Frame(body);search.grid(row=1,column=0,sticky='ew',pady=12);search.columnconfigure(1,weight=1)
        ttk.Label(search,text='Search').grid(row=0,column=0,padx=(0,10));entry=ttk.Entry(search,textvariable=self.query);entry.grid(row=0,column=1,sticky='ew');ttk.Label(search,textvariable=self.count).grid(row=0,column=2,padx=(12,0))
        table=ttk.Frame(body);table.grid(row=2,column=0,sticky='nsew');table.columnconfigure(0,weight=1);table.rowconfigure(0,weight=1)
        self.tree=ttk.Treeview(table,columns=('ig','name'),show='headings',selectmode='browse');self.tree.heading('ig',text='IG number');self.tree.heading('name',text='IG / agency name');self.tree.column('ig',width=110,stretch=False);self.tree.column('name',width=550);self.tree.grid(row=0,column=0,sticky='nsew')
        scroll=ttk.Scrollbar(table,orient='vertical',command=self.tree.yview);scroll.grid(row=0,column=1,sticky='ns');self.tree.configure(yscrollcommand=scroll.set);self.tree.bind('<<TreeviewSelect>>',self.select)
        fields=ttk.LabelFrame(body,text='Add or edit a mapping',padding=12);fields.grid(row=3,column=0,sticky='ew',pady=14);fields.columnconfigure(1,weight=1)
        ttk.Label(fields,text='IG number').grid(row=0,column=0,sticky='w',padx=(0,12));ttk.Entry(fields,textvariable=self.code,width=15).grid(row=0,column=1,sticky='w')
        ttk.Label(fields,text='IG / agency name').grid(row=1,column=0,sticky='w',pady=10,padx=(0,12));ttk.Entry(fields,textvariable=self.name).grid(row=1,column=1,sticky='ew',pady=10)
        actions=ttk.Frame(fields);actions.grid(row=2,column=0,columnspan=2,sticky='w')
        for label,command in [('New / clear',self.clear),('Add',self.add),('Update selected',self.update_selected),('Remove selected',self.remove)]:IconButton(actions,text=label,command=command).pack(side='left',padx=(0,8))
        ttk.Label(body,text='Changes apply to the next report run. Removing a mapping keeps its transactions;\nthe agency label will show the IG number as unmapped.',wraplength=730).grid(row=4,column=0,sticky='w')
        footer=ttk.Frame(body);footer.grid(row=5,column=0,sticky='ew',pady=(16,0));IconButton(footer,text='Save mappings',style='Accent.TButton',command=self.save).pack(side='right');IconButton(footer,text='Cancel',command=self.close).pack(side='right',padx=8)
        self.query.trace_add('write',lambda *_:self.refresh());self.refresh()
        self.update_idletasks();x=max(0,min(parent.winfo_rootx()+(parent.winfo_width()-790)//2,self.winfo_screenwidth()-790));y=max(0,min(parent.winfo_rooty()+(parent.winfo_height()-650)//2,self.winfo_screenheight()-680));self.geometry(f'790x650+{x}+{y}');entry.focus_set()
    def refresh(self):
        self.tree.delete(*self.tree.get_children());q=self.query.get().strip().casefold();visible=0
        for k,v in sorted(self.mapping.items(),key=lambda kv:int(kv[0])):
            if not q or q in k.casefold() or q in v.casefold():self.tree.insert('','end',iid=k,values=(k,v));visible+=1
        self.count.set(f'{visible} / {len(self.mapping)} mappings')
    def select(self,_=None):
        selected=self.tree.selection()
        if not selected:return
        self.selected=selected[0];self.code.set(self.selected);self.name.set(self.mapping[self.selected])
    def clear(self):
        self.selected=None;self.code.set('');self.name.set('');self.tree.selection_remove(*self.tree.selection())
    def values(self):
        k=self.code.get().strip();v=self.name.get().strip()
        if not re.fullmatch(r'[1-9]\d{0,7}',k):raise ValueError('Enter a positive whole IG number (up to 8 digits, no leading zeros).')
        if not v:raise ValueError('Enter an IG / agency name.')
        return k,v
    def add(self):
        try:
            k,v=self.values()
            if k in self.mapping:raise ValueError('This IG number already exists. Select it and use Update selected.')
            self.mapping[k]=v;self.refresh();self.selected=k
            if self.tree.exists(k):self.tree.selection_set(k);self.tree.see(k)
        except ValueError as e:messagebox.showerror('Check mapping',str(e),parent=self)
    def update_selected(self):
        try:
            if self.selected not in self.mapping:raise ValueError('Select a mapping from the list first.')
            k,v=self.values()
            if k!=self.selected and k in self.mapping:raise ValueError('Another mapping already uses that IG number.')
            self.mapping.pop(self.selected);self.mapping[k]=v;self.selected=k;self.refresh()
            if self.tree.exists(k):self.tree.selection_set(k);self.tree.see(k)
        except ValueError as e:messagebox.showerror('Check mapping',str(e),parent=self)
    def remove(self):
        if self.selected not in self.mapping:messagebox.showinfo('Select an IG','Select a mapping to remove.',parent=self);return
        self.mapping.pop(self.selected);self.clear();self.refresh()
    def save(self):
        try:
            save_agencies(self.path,self.mapping);self.original=dict(self.mapping);self.destroy()
        except Exception as e:messagebox.showerror('Could not save mappings',str(e),parent=self)
    def close(self):
        if self.mapping!=self.original and not messagebox.askyesno('Discard changes?','Close without saving your IG changes?',parent=self):return
        self.destroy()
