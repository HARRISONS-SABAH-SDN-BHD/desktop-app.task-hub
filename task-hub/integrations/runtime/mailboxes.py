"""TLS-only mail server adapters. Never delete messages or mark them read."""
import email, imaplib, poplib, smtplib, ssl, re
from email import policy
from email.parser import BytesParser
from email.header import decode_header, make_header
from email.utils import parseaddr
from pathlib import Path
MAX_MESSAGE=100*1024*1024

def text(value):
    try:return str(make_header(decode_header(value or '')))[:500]
    except Exception:return str(value or '')[:500]

def validate(config,password):
    if config.get('protocol') not in ['imap','pop3']:raise ValueError('Choose IMAP or POP3.')
    for key in ['host','smtp_host']:
        host=config.get(key,'')
        if key=='smtp_host' and not host:continue
        if len(host)>253 or not re.fullmatch(r'[A-Za-z0-9.-]+',host) or host.startswith('.') or '..' in host:raise ValueError('Use mail server hostnames without URLs, spaces or paths.')
    for key in ['port','smtp_port']:
        if not isinstance(config.get(key),int) or not 1<=config[key]<=65535:raise ValueError('Use valid mail server port numbers.')
    for key in ['username','smtp_username','folder','draft_folder']:
        value=config.get(key,'')
        if len(value)>512 or any(ord(c)<32 for c in value):raise ValueError('Mail account fields contain invalid characters.')
    if not password or len(password)>2048:raise ValueError('Enter the account password or provider-issued app password.')
    if config.get('smtp_security') not in ['tls','starttls']:raise ValueError('SMTP requires TLS or STARTTLS.')

def incoming(config,password):
    validate(config,password)
    if config['protocol']=='imap':
        client=imaplib.IMAP4_SSL(config['host'],config['port'],ssl_context=ssl.create_default_context(),timeout=30)
        try:client.login(config['username'],password)
        except Exception:client.shutdown();raise ValueError('IMAP authentication failed. Check server details and app-password requirements.')
        return client
    client=poplib.POP3_SSL(config['host'],config['port'],timeout=30,context=ssl.create_default_context())
    try:client.user(config['username']);client.pass_(password)
    except Exception:client.close();raise ValueError('POP3 authentication failed. Check server details and app-password requirements.')
    return client

def close(client,protocol):
    try:client.logout() if protocol=='imap' else client.quit()
    except Exception:pass

def smtp(config,password):
    if not config.get('smtp_host'):raise ValueError('Add outgoing SMTP settings to send email.')
    context=ssl.create_default_context()
    if config['smtp_security']=='tls':client=smtplib.SMTP_SSL(config['smtp_host'],config['smtp_port'],timeout=30,context=context)
    else:
        client=smtplib.SMTP(config['smtp_host'],config['smtp_port'],timeout=30);client.ehlo();client.starttls(context=context);client.ehlo()
    try:client.login(config.get('smtp_username') or config['username'],password)
    except Exception:client.close();raise ValueError('SMTP authentication failed. Check outgoing server settings.')
    return client

def connect(config,password):
    client=incoming(config,password);close(client,config['protocol'])
    if config.get('smtp_host'):
        client=smtp(config,password);client.noop();client.quit()
    return {'connected':True}

def imap_select(client,folder):
    # Explicit quoting prevents injected protocol tokens; no writable selection.
    quoted='"'+folder.replace('\\','\\\\').replace('"','\\"')+'"'
    status,_=client.select(quoted,readonly=True)
    if status!='OK':raise ValueError('IMAP folder could not be opened. Check the incoming folder name.')
    validity=client.response('UIDVALIDITY')[1]
    if not validity or not validity[0]:raise ValueError('IMAP folder did not provide a stable UIDVALIDITY.')
    return validity[0].decode('ascii')

def pop_ids(client):
    result={}
    for line in client.uidl()[1]:
        index,uid=line.decode('ascii').split(' ',1);result[uid]=int(index)
    return result

def message_info(raw,key):
    message=BytesParser(policy=policy.default).parsebytes(raw,headersonly=True)
    return {'id':key,'subject':text(message.get('Subject')),'sender':text(message.get('From')),'received':text(message.get('Date'))}

def messages(config,password):
    client=incoming(config,password)
    try:
        result=[]
        if config['protocol']=='imap':
            validity=imap_select(client,config.get('folder') or 'INBOX');status,data=client.uid('search',None,'ALL')
            if status!='OK':raise ValueError('Could not search the IMAP folder.')
            for uid in reversed((data[0] or b'').split()[-50:]):
                status,data=client.uid('fetch',uid,'(BODY.PEEK[HEADER.FIELDS (SUBJECT FROM DATE)])')
                if status=='OK':
                    raw=b''.join(item[1] for item in data if isinstance(item,tuple));result.append(message_info(raw,validity+':'+uid.decode('ascii')))
        else:
            for uid,index in list(pop_ids(client).items())[-50:][::-1]:
                try:raw=b'\r\n'.join(client.top(index,0)[1])
                except poplib.error_proto:raise ValueError('This POP3 server does not support header-only browsing (TOP). Use IMAP.')
                result.append(message_info(raw,uid))
        return result
    finally:close(client,config['protocol'])

def raw_message(config,password,key):
    client=incoming(config,password)
    try:
        if config['protocol']=='imap':
            validity=imap_select(client,config.get('folder') or 'INBOX')
            parts=key.split(':')
            if len(parts)!=2 or parts[0]!=validity or not parts[1].isdigit():raise ValueError('The IMAP folder changed. Refresh messages before selecting an attachment.')
            status,data=client.uid('fetch',parts[1],'(RFC822.SIZE)')
            match=re.search(rb'RFC822.SIZE (\d+)',b' '.join(d for d in data if isinstance(d,bytes)))
            if status!='OK' or not match or int(match[1])>MAX_MESSAGE:raise ValueError('The message is unavailable or larger than 100 MB.')
            status,data=client.uid('fetch',parts[1],'(BODY.PEEK[])')
            if status!='OK':raise ValueError('Could not read the selected IMAP message.')
            raw=b''.join(d[1] for d in data if isinstance(d,tuple))
        else:
            ids=pop_ids(client)
            if key not in ids:raise ValueError('POP3 message moved or was deleted. Refresh the message list.')
            index=ids[key];sizes={int(line.split()[0]):int(line.split()[1]) for line in client.list()[1]}
            if sizes.get(index,MAX_MESSAGE+1)>MAX_MESSAGE:raise ValueError('The selected message is larger than 100 MB.')
            raw=b'\r\n'.join(client.retr(index)[1])
        if len(raw)>MAX_MESSAGE:raise ValueError('The selected message exceeds 100 MB.')
        return raw
    finally:close(client,config['protocol'])

def attachments(config,password,key,index=None,destination=None):
    message=BytesParser(policy=policy.default).parsebytes(raw_message(config,password,key));items=[]
    for position,part in enumerate(message.walk()):
        name=text(part.get_filename())
        if not name.lower().endswith(('.xlsx','.xlsm')):continue
        if any(c in name for c in '\\/:*?"<>|') or name in ['.','..']:continue
        data=part.get_payload(decode=True) or b''
        if len(data)>MAX_MESSAGE:raise ValueError('Workbook attachment exceeds 100 MB.')
        items.append({'id':str(position),'name':name,'size':len(data)})
        if index is not None and str(position)==str(index):
            Path(destination).write_bytes(data);return {'name':name,'size':len(data)}
    if index is not None:raise ValueError('Selected workbook attachment is no longer available.')
    return items

def deliver(config,password,raw,recipient,cc,send):
    if send:
        client=smtp(config,password)
        try:
            refused=client.sendmail(config['email'],[recipient]+cc,raw)
            if refused:raise ValueError('Some recipients were refused. Inspect your provider before retrying; no retry was attempted.')
        except Exception:raise ValueError('SMTP submission failed or its outcome is uncertain. Inspect your provider before retrying. No retry was attempted.')
        finally:
            try:client.quit()
            except Exception:pass
        return {'accepted':True,'detail':'SMTP accepted the message. SMTP does not automatically create a Sent Items copy.'}
    if config['protocol']!='imap':raise ValueError('POP3 cannot create server-side drafts. Choose automatic sending or use an IMAP or Gmail connection for drafts.')
    client=incoming(config,password)
    try:
        folder='"'+(config.get('draft_folder') or 'Drafts').replace('\\','\\\\').replace('"','\\"')+'"'
        status,_=client.append(folder,'\\Draft',None,raw)
        if status!='OK':raise ValueError('IMAP draft was not confirmed. Inspect Drafts before retrying.')
        return {'accepted':True,'detail':'Draft saved to the configured IMAP Drafts folder. Nothing was sent.'}
    finally:close(client,'imap')
