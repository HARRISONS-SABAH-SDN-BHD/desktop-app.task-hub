"""Small native vector widgets for the reference layout; no image downloads."""
import math
import tkinter as tk
from tkinter import ttk,font

BG='#EAF0F6';WHITE='#FFFFFF';INK='#12263D';MUTED='#526780';TEAL='#006A70';BORDER='#C8D5E0'

def rounded(canvas,x1,y1,x2,y2,r=8,**kw):
    return canvas.create_polygon(x1+r,y1,x2-r,y1,x2,y1,x2,y1+r,x2,y2-r,x2,y2,x2-r,y2,x1+r,y2,x1,y2,x1,y2-r,x1,y1+r,x1,y1,smooth=True,splinesteps=20,**kw)

def icon(c,kind,x,y,color,size=20):
    scale=size/24
    def points(values):return [v*scale+(x if i%2==0 else y) for i,v in enumerate(values)]
    def line(*v):c.create_line(*points(v),fill=color,width=max(1.5,scale*1.7),capstyle='round',joinstyle='round')
    def rect(a,b,d,e):c.create_rectangle(*points([a,b,d,e]),outline=color,width=max(1.5,scale*1.7))
    if kind=='home':
        line(1,11,12,2,23,11);line(5,9,5,23,19,23,19,9);line(10,23,10,15,15,15,15,23)
    elif kind=='folder':
        line(2,7,2,20,22,20,22,7,11,7,8,4,2,4,2,7);line(2,10,22,10)
    elif kind=='play':c.create_polygon(*points([5,3,21,12,5,21]),outline=color,fill='',width=2)
    elif kind=='settings':
        c.create_oval(*points([6,6,18,18]),outline=color,width=2);c.create_oval(*points([10,10,14,14]),outline=color,width=1.5)
        for i in range(8):
            a=i*math.pi/4;line(12+6*math.cos(a),12+6*math.sin(a),12+10*math.cos(a),12+10*math.sin(a))
    elif kind=='users':
        c.create_oval(*points([7,1,15,9]),outline=color,width=1.7);line(4,20,4,16,7,12,15,12,18,16,18,20);rect(14,16,23,23)
    elif kind=='open':
        line(11,3,3,3,3,21,21,21,21,13);line(12,2,23,2,23,12);line(10,15,23,2)
    else:
        line(4,1,15,1,22,8,22,23,4,23,4,1);line(15,1,15,8,22,8)
        if kind in ['excel','sheet']:
            rect(7,11,18,20);line(7,14,18,14);line(7,17,18,17);line(11,11,11,20)
        elif kind=='preview':
            c.create_oval(*points([11,12,20,21]),outline=color,width=1.8);line(19,20,24,25)

class Symbol(tk.Canvas):
    def __init__(self,parent,kind='sheet',color=TEAL,size=24,bg=WHITE,**kw):
        super().__init__(parent,width=size+6,height=size+6,bg=bg,highlightthickness=0,**kw);icon(self,kind,3,3,color,size)

class Card(tk.Frame):
    def __init__(self,parent,padding=18,**kw):
        super().__init__(parent,bg=BG,**kw)
        self.canvas=tk.Canvas(self,bg=BG,highlightthickness=0,bd=0);self.canvas.place(relwidth=1,relheight=1)
        self.content=ttk.Frame(self,style='Card.TFrame',padding=max(0,padding-10));self.content.pack(fill='both',expand=True,padx=10,pady=10)
        self.bind('<Configure>',lambda e:self.draw(e.width,e.height))
    def draw(self,w,h):
        self.canvas.delete('all');rounded(self.canvas,1,2,w-1,h-1,12,fill='#DFE7EF',outline='');rounded(self.canvas,1,1,w-1,h-3,12,fill=WHITE,outline=BORDER,width=1)

class IconButton(tk.Canvas):
    def __init__(self,parent,text='',command=None,kind=None,style='',state='normal',width=None,**kw):
        self._state=state;self._hover=False;self.text=text;self.command=command;self.kind=kind;self.primary='Accent' in style
        self.font=font.Font(family='Segoe UI',size=11,weight='bold' if self.primary else 'normal')
        natural=self.font.measure(text)+40+(28 if kind else 0)
        parentbg=parent.cget('background') if isinstance(parent,tk.Frame) else ttk.Style(parent).lookup(parent.cget('style') or 'TFrame','background') or BG
        super().__init__(parent,width=width or natural,height=42,highlightthickness=0,bd=0,bg=parentbg,takefocus=True,cursor='hand2',**kw)
        self.bind('<Configure>',lambda _:self.draw());self.bind('<Enter>',lambda _:self.hover(True));self.bind('<Leave>',lambda _:self.hover(False))
        self.bind('<Button-1>',self.invoke);self.bind('<Return>',self.invoke);self.bind('<space>',self.invoke);self.bind('<FocusIn>',lambda _:self.draw());self.bind('<FocusOut>',lambda _:self.draw());self.draw()
    def configure(self,cnf=None,**kw):
        if 'state' in kw:self._state=kw.pop('state');self.draw()
        return super().configure(cnf,**kw) if cnf or kw else None
    config=configure
    def hover(self,value):self._hover=value;self.draw()
    def invoke(self,_=None):
        if self._state!='disabled' and self.command:self.focus_set();self.command()
        return 'break'
    def draw(self):
        self.delete('all');w=int(float(self.cget('width'))) if self.winfo_width()<2 else self.winfo_width();h=42
        disabled=self._state=='disabled';fg='#8395A6' if disabled else WHITE if self.primary else INK
        fill=('#C6DADD' if self.primary else '#EDF2F7') if disabled else '#005C63' if self.primary and self._hover else TEAL if self.primary else '#E5EDF4' if self._hover else '#F7FAFD'
        rounded(self,1,1,w-1,h-1,14,fill=fill,outline=TEAL if self.primary and not disabled else '#B9CAD9',width=1)
        total=self.font.measure(self.text)+(28 if self.kind else 0);start=(w-total)/2
        if self.kind:icon(self,self.kind,start,10,fg,20);start+=28
        self.create_text(start,h/2,text=self.text,font=self.font,fill=fg,anchor='w')
        if self.focus_get()==self:rounded(self,4,4,w-4,h-4,11,fill='',outline='#76AEB6',dash=(2,2))

class ChoiceButton(IconButton):
    def __init__(self,parent,variable,value,**kw):
        self.variable=variable;self.value=value
        super().__init__(parent,text=value,**kw)
        self._trace=variable.trace_add('write',lambda *_:self.draw())
        self.bind('<Destroy>',self.cleanup,add='+')
    def draw(self):
        self.primary=self.variable.get()==self.value
        if hasattr(self,'font'):self.font.configure(weight='bold' if self.primary else 'normal')
        super().draw()
    def invoke(self,_=None):
        if self._state!='disabled':self.focus_set();self.variable.set(self.value)
        return 'break'
    def cleanup(self,event):
        if event.widget==self:
            try:self.variable.trace_remove('write',self._trace)
            except tk.TclError:pass

class ChoiceGroup(ttk.Frame):
    """Three rounded choices sharing the same application variable."""
    def __init__(self,parent,variable,values):
        super().__init__(parent,style='Card.TFrame');self.variable=variable;self.values=list(values);self.buttons=[]
        for i,value in enumerate(values):
            self.columnconfigure(i,weight=1,uniform='choices')
            button=ChoiceButton(self,variable,value,width=85)
            button.grid(row=0,column=i,sticky='ew',padx=(0,7 if i<len(values)-1 else 0));self.buttons.append(button)
            button.bind('<Left>',lambda _,step=-1:self.move(step));button.bind('<Right>',lambda _,step=1:self.move(step))
    def move(self,step):
        if any(b._state=='disabled' for b in self.buttons):return 'break'
        i=self.values.index(self.variable.get()) if self.variable.get() in self.values else 0
        i=(i+step)%len(self.values);self.buttons[i].focus_set();self.variable.set(self.values[i]);return 'break'

def section(parent,number,text,row=0,span=2):
    header=ttk.Frame(parent,style='Card.TFrame');header.grid(row=row,column=0,columnspan=span,sticky='ew',pady=(0,14))
    badge=tk.Canvas(header,width=38,height=30,bg=WHITE,highlightthickness=0);badge.pack(side='left',padx=(0,12));rounded(badge,0,0,38,30,7,fill=TEAL,outline='');badge.create_text(19,15,text=number,fill=WHITE,font=('Segoe UI',12,'bold'))
    ttk.Label(header,text=text,style='Section.TLabel').pack(side='left')
