"""Bounded server diagnostics. Never return a protocol transcript or credential."""
import base64,json,re
from urllib.parse import quote,quote_plus

def server_reply(error,config,password=None):
    import poplib,imaplib,smtplib
    if isinstance(error,smtplib.SMTPResponseException):value=error.smtp_error
    elif isinstance(error,(poplib.error_proto,imaplib.IMAP4.error)):value=error.args[0] if error.args else ''
    else:return ''
    if isinstance(value,bytes):
        if len(value)>65536:return '[server reply omitted because it is too large]'
        value=value.decode('utf-8',errors='replace')
    if not isinstance(value,str):return ''
    if len(value)>65536:return '[server reply omitted because it is too large]'
    secrets={str(v) for v in [password,config.get('username'),config.get('smtp_username'),config.get('email')] if v}
    if password:
        for user in [config.get('username'),config.get('smtp_username')]:
            if user:secrets.update([f'{user}:{password}',f'\0{user}\0{password}',f'{user}\0{user}\0{password}'])
    variants=set()
    for secret in secrets:
        variants.update([secret,quote(secret,safe=''),quote_plus(secret,safe=''),json.dumps(secret,ensure_ascii=True)[1:-1],repr(secret)[1:-1],base64.b64encode(secret.encode()).decode(),base64.urlsafe_b64encode(secret.encode()).decode()])
    def redact(text):
        for secret in sorted(variants,key=len,reverse=True):
            if secret:text=re.sub(re.escape(secret),'[redacted]',text,flags=re.IGNORECASE)
        return text
    value=redact(value)
    value=re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]','',value)
    value=''.join(c if c.isprintable() else ' ' for c in value)
    value=redact(value)
    value=re.sub(r'(?i)\b(Bearer|Basic)\s+[^\s,;]+',r'\1 [redacted]',value)
    value=re.sub(r"(?i)\b(password|passwd|token|secret|authorization)\s*[:=]\s*(?:\"[^\"]*\"|'[^']*'|[^\s,;]+)",r'\1=[redacted]',value)
    value=' '.join(value.split())
    return value[:500]+('…' if len(value)>500 else '')
