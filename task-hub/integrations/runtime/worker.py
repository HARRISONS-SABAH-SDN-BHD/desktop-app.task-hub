"""Task Hub worker protocol. One bounded JSON request; no shell, no secret logging."""
import base64,json,sys,traceback
from pathlib import Path
BASE=Path(sys.executable).parent if getattr(sys,'frozen',False) else Path(__file__).resolve().parent
sys.path.insert(0,str(Path(__file__).resolve().parent/'vendor'))
def progress(message,percent):print(json.dumps({'type':'progress','message':str(message)[:200],'percent':percent}),flush=True)
def dispatch(req):
    operation=req.get('operation')
    if operation=='ping':return {'version':1}
    if operation.startswith('mail_'):
        import mailboxes
        config=req['config'];password=req['password'];mailboxes.validate(config,password)
        if operation=='mail_connect':return mailboxes.connect(config,password)
        if operation=='mail_messages':return mailboxes.messages(config,password)
        if operation=='mail_attachments':return mailboxes.attachments(config,password,req['message_id'])
        if operation=='mail_download':return mailboxes.attachments(config,password,req['message_id'],req['attachment_id'],req['destination'])
        if operation=='mail_deliver':return mailboxes.deliver(config,password,base64.b64decode(req['raw']),req['recipient'],req['cc'],req['send'])
    if operation in ['generate','settings','save_agencies']:
        import engine
        from decimal import Decimal
        config=Path(req['settings_path'])
        if not config.exists():config.parent.mkdir(parents=True,exist_ok=True);config.write_bytes((BASE/'settings.json').read_bytes())
        settings=engine.load_settings(config)
        if operation=='settings':return {'agencies':settings['agencies'],'branches':settings['branches']}
        if operation=='save_agencies':engine.save_agencies(config,req['agencies']);return {'saved':True}
        paths=engine.selected_input_paths({k:Path(v) for k,v in req['inputs'].items()},req.get('scope','Both'))
        if req.get('auto_period',True):month,year=engine.period_from_inputs(paths,require_both=False)
        else:month,year=int(req['month']),int(req['year'])
        if not 1<=month<=12 or not 1900<=year<=2200:raise ValueError('Choose a valid report month and year.')
        money=int(req.get('money_places',2));quantity=int(req.get('quantity_places',2));factor=Decimal(str(req.get('factor','0.88')))
        if not 0<=money<=6 or not 0<=quantity<=8 or not factor.is_finite() or not Decimal(0)<=factor<=Decimal(10):raise ValueError('Invalid decimal precision or multiplier.')
        mode=req.get('mode','Both')
        if mode not in ['Both','Normal','Special']:raise ValueError('Choose Both, Normal or Special report versions.')
        options=engine.Options(month,year,mode,money,quantity,bool(req.get('repair_dates',True)),factor)
        batch=engine.Batch()
        for kind,path in paths.items():
            if not path.is_file() or path.suffix.lower() not in ['.xlsx','.xlsm']:raise ValueError('Choose an existing Excel workbook for each input.')
            engine.read_input(path,kind,options,settings,batch,lambda m,p:progress(m,None if p is None else p*.3))
        if batch.errors:raise ValueError(f'{batch.errors} validation errors. Correct the raw workbooks in ExcelFlow before generating reports. No output email was prepared.')
        details={'preview':batch.preview[:100], 'issues':batch.issues[:1000], 'inputs':batch.stats}
        if req.get('validate_only'):return {'validated':True,'period':f'{year:04}-{month:02}','warnings':len(batch.issues),'details':details}
        root,summary=engine.generate(batch,options,settings,Path(req['destination']),lambda m,p:progress(m,None if p is None else 30+p*.7))
        return {'folder':str(root),'summary':summary,'details':details}
    raise ValueError('Unsupported Task Hub worker operation.')
def main():
    req={}
    try:
        text=sys.stdin.buffer.read(40*1024*1024+1)
        if len(text)>40*1024*1024:raise ValueError('Worker request is too large.')
        req=json.loads(text);result=dispatch(req);print(json.dumps({'type':'result','data':result},default=str),flush=True)
    except Exception as error:
        # Protocol/server exception strings can contain credentials; emit only known validation messages.
        if isinstance(error,ValueError) or (type(error).__name__=='InvalidData' and type(error).__module__=='engine'):message=str(error)[:1000]
        elif isinstance(req,dict) and str(req.get('operation','')).startswith('mail_'):
            import mailboxes
            config=req.get('config',{});message=str(mailboxes.connection_error(str(config.get('protocol','mail')).upper(),'mailbox operation',config,error,password=req.get('password')))[:1000]
        else:message='The operation could not complete. Check the connection, input files and permissions.'
        print(json.dumps({'type':'error','message':message}),flush=True);return 1
    return 0
if __name__=='__main__':raise SystemExit(main())
