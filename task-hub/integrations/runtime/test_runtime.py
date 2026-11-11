import hashlib
import json,sys,unittest,tempfile,ssl
from pathlib import Path
from unittest.mock import Mock,patch
sys.path.insert(0,str(Path(__file__).parent))
import mailboxes,worker
class MailTests(unittest.TestCase):
 def config(self,protocol='imap'):
  return dict(email='sender@example.com',protocol=protocol,host='mail.example.com',port=993 if protocol=='imap' else 995,username='user',folder='INBOX',draft_folder='Drafts',smtp_host='',smtp_port=587,smtp_username='',smtp_security='starttls')
 def test_imap_auth_is_tls_and_header_reads_are_peek_and_readonly(self):
  client=Mock();client.select.return_value=('OK',[]);client.response.return_value=('UIDVALIDITY',[b'77']);client.uid.side_effect=[('OK',[b'1']),('OK',[(b'header',b'Subject: Report\r\nFrom: test@example.com\r\nDate: Tue, 10 Nov 2026 10:00:00 +0800\r\n\r\n')])]
  with patch('mailboxes.imaplib.IMAP4_SSL',return_value=client) as constructor:
   result=mailboxes.messages(self.config(),'secret-only-in-test')
  self.assertIsInstance(constructor.call_args.kwargs['ssl_context'],ssl.SSLContext);client.select.assert_called_with('"INBOX"',readonly=True);self.assertIn('PEEK',client.uid.call_args.args[2]);self.assertEqual(result[0]['id'],'77:1');client.delete.assert_not_called();client.store.assert_not_called()
 def test_pop_headers_never_retrieve_or_delete(self):
  client=Mock();client.uidl.return_value=(b'OK',[b'1 stable-id'],10);client.top.return_value=(b'OK',[b'Subject: Report'],10)
  with patch('mailboxes.poplib.POP3_SSL',return_value=client):result=mailboxes.messages(self.config('pop3'),'password')
  self.assertEqual(result[0]['id'],'stable-id');client.retr.assert_not_called();client.dele.assert_not_called();client.quit.assert_called_once()
 def test_changed_imap_uidvalidity_stops_before_body_read(self):
  client=Mock();client.select.return_value=('OK',[]);client.response.return_value=('UIDVALIDITY',[b'new'])
  with patch('mailboxes.imaplib.IMAP4_SSL',return_value=client),self.assertRaises(ValueError):mailboxes.raw_message(self.config(),'password','old:1')
  client.uid.assert_not_called()
 def test_bad_hosts_and_credentials_rejected(self):
  c=self.config();c['host']='https://example.com/path'
  with self.assertRaises(ValueError):mailboxes.validate(c,'password')
 def test_smtp_submission_is_once_and_partial_refusal_is_not_success(self):
  client=Mock();client.sendmail.return_value={'copy@example.com':(550,b'refused')}
  with patch('mailboxes.smtp',return_value=client),self.assertRaises(ValueError):mailboxes.deliver(self.config(),'password',b'Raw','to@example.com',['copy@example.com'],True)
  client.sendmail.assert_called_once()
 def test_vendored_engine_matches_its_revision_record(self):
  base=Path(__file__).parent;origin=json.loads((base/'vendor/origin.json').read_text());self.assertEqual(hashlib.sha256((base/'vendor/engine.py').read_bytes()).hexdigest(),origin['sha256'])
 def test_worker_unknown_operation_fails(self):
  with self.assertRaises(ValueError):worker.dispatch({'operation':'arbitrary-command'})
class PopSecurityTests(unittest.TestCase):
 config = MailTests.config
 def test_starttls_precedes_all_credentials_and_verifies_certificates(self):
  c=self.config('pop3');c.update(port=110,incoming_security='starttls');client=Mock();calls=[]
  client.stls.side_effect=lambda **kwargs:calls.append(('tls',kwargs['context']))
  client.user.side_effect=lambda value:calls.append(('user',value));client.pass_.side_effect=lambda value:calls.append(('password',value))
  with patch('mailboxes.poplib.POP3',return_value=client),patch('mailboxes.poplib.POP3_SSL') as implicit:
   self.assertIs(mailboxes.incoming(c,'test-password'),client)
  self.assertEqual([v[0] for v in calls],['tls','user','password']);self.assertEqual(calls[0][1].verify_mode,ssl.CERT_REQUIRED);self.assertTrue(calls[0][1].check_hostname);implicit.assert_not_called()
 def test_failed_starttls_never_sends_credentials_or_downgrades(self):
  c=self.config('pop3');c.update(port=110,incoming_security='starttls');client=Mock();client.stls.side_effect=mailboxes.poplib.error_proto(b'-ERR not supported test-password')
  with patch('mailboxes.poplib.POP3',return_value=client),self.assertRaises(mailboxes.MailConnectionError) as result:mailboxes.incoming(c,'test-password')
  self.assertIn('STARTTLS',str(result.exception));self.assertIn('No username or password was sent',str(result.exception));self.assertNotIn('test-password',str(result.exception));client.user.assert_not_called();client.pass_.assert_not_called();client.close.assert_called_once()
 def test_none_is_explicit_and_does_not_start_tls(self):
  c=self.config('pop3');c.update(port=110,incoming_security='none');client=Mock()
  with patch('mailboxes.poplib.POP3',return_value=client) as clear,patch('mailboxes.poplib.POP3_SSL') as encrypted:mailboxes.incoming(c,'test-password')
  clear.assert_called_once();encrypted.assert_not_called();client.stls.assert_not_called();client.user.assert_called_once_with('user');client.pass_.assert_called_once_with('test-password')
 def test_legacy_saved_account_still_uses_implicit_tls(self):
  c=self.config('pop3');client=Mock()
  with patch('mailboxes.poplib.POP3_SSL',return_value=client) as encrypted,patch('mailboxes.poplib.POP3') as clear:mailboxes.incoming(c,'test-password')
  encrypted.assert_called_once();clear.assert_not_called()
 def test_dns_timeout_and_certificate_failures_are_specific_and_safe(self):
  for error,expected in [(mailboxes.socket.gaierror('secret-password'),'hostname could not be resolved'),(TimeoutError('secret-password'),'timed out'),(ConnectionRefusedError('secret-password'),'refused the connection'),(ssl.SSLCertVerificationError('secret-password'),'certificate could not be verified'),(ssl.SSLError('secret-password'),'TLS negotiation failed')]:
   with self.subTest(expected=expected),patch('mailboxes.poplib.POP3_SSL',side_effect=error),self.assertRaises(mailboxes.MailConnectionError) as result:mailboxes.incoming(self.config('pop3'),'secret-password')
   message=str(result.exception);self.assertIn('mail.example.com:995',message);self.assertIn(expected,message);self.assertNotIn('secret-password',message)
 def test_rejected_password_is_not_reported_as_network_error(self):
  client=Mock();client.pass_.side_effect=mailboxes.poplib.error_proto(b'-ERR echoed-secret')
  with patch('mailboxes.poplib.POP3_SSL',return_value=client),self.assertRaises(mailboxes.MailConnectionError) as result:mailboxes.incoming(self.config('pop3'),'echoed-secret')
  self.assertIn('POP3 sign-in (PASS) failed',str(result.exception));self.assertNotIn('echoed-secret',str(result.exception));client.close.assert_called_once()
 def test_incoming_success_and_smtp_failure_are_distinguished(self):
  c=self.config();c['smtp_host']='smtp.example.com';failure=mailboxes.connection_error('SMTP','sign-in',c,mailboxes.smtplib.SMTPAuthenticationError(535,b'echoed-secret'),True,password='echoed-secret')
  with patch('mailboxes.incoming',return_value=Mock()),patch('mailboxes.smtp',side_effect=failure),self.assertRaises(mailboxes.MailConnectionError) as result:mailboxes.connect(c,'echoed-secret')
  self.assertIn('Incoming IMAP sign-in succeeded',str(result.exception));self.assertIn('SMTP status 535',str(result.exception));self.assertNotIn('echoed-secret',str(result.exception))
 def test_imap_and_unknown_modes_cannot_select_unencrypted_access(self):
  for protocol,security in [('imap','none'),('imap','starttls'),('pop3','unknown')]:
   c=self.config(protocol);c['incoming_security']=security
   with self.assertRaises(ValueError):mailboxes.validate(c,'password')
 def test_real_plain_pop3_socket_uses_only_selected_none_mode(self):
  import socketserver,threading
  commands=[]
  class Handler(socketserver.StreamRequestHandler):
   def handle(self):
    self.wfile.write(b'+OK test server ready\r\n')
    while True:
     line=self.rfile.readline().decode().strip();commands.append(line.split(' ')[0])
     if not line:break
     self.wfile.write(b'+OK accepted\r\n')
     if line=='QUIT':break
  server=socketserver.TCPServer(('127.0.0.1',0),Handler);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
  try:
   c=self.config('pop3');c.update(host='127.0.0.1',port=server.server_address[1],incoming_security='none');self.assertTrue(mailboxes.connect(c,'local-test-only')['connected'])
   self.assertEqual(commands,['USER','PASS','QUIT'])
  finally:server.shutdown();server.server_close();thread.join(3)

class ServerReplyTests(unittest.TestCase):
 config=MailTests.config
 def test_mailbox_lock_reason_reaches_display(self):
  c=self.config('pop3');client=Mock();client.pass_.side_effect=mailboxes.poplib.error_proto(b'-ERR [IN-USE] Maildrop already locked by another session')
  with patch('mailboxes.poplib.POP3_SSL',return_value=client),self.assertRaises(mailboxes.MailConnectionError) as result:mailboxes.incoming(c,'test-password')
  message=str(result.exception);self.assertIn('sign-in (PASS)',message);self.assertIn('Server reply: -ERR [IN-USE] Maildrop already locked',message)
 def test_plain_encoded_and_escaped_credentials_are_redacted(self):
  import base64,json
  from urllib.parse import quote
  password='private p@ss!';c=self.config('pop3');user=c['username']
  values=[password,quote(password,safe=''),base64.b64encode(password.encode()).decode(),base64.b64encode(('\0'+user+'\0'+password).encode()).decode(),json.dumps(password)[1:-1],user,c['email']]
  for secret in values:
   with self.subTest(secret=secret):
    error=mailboxes.poplib.error_proto(('-ERR rejected '+secret).encode());result=str(mailboxes.connection_error('POP3','password',c,error,password=password));self.assertNotIn(secret,result);self.assertIn('[redacted]',result)
 def test_smtp_server_reason_and_status_survive_without_credentials(self):
  c=self.config();c['smtp_host']='smtp.example.com'
  result=str(mailboxes.connection_error('SMTP','sign-in',c,mailboxes.smtplib.SMTPAuthenticationError(535,b'5.7.8 Login denied token=server-token'),True,password='local-password'))
  self.assertIn('535',result);self.assertIn('5.7.8 Login denied',result);self.assertNotIn('server-token',result)
 def test_controls_are_removed_and_replies_are_bounded(self):
  c=self.config('pop3');error=mailboxes.poplib.error_proto(b'-ERR\r\n\x1b[31mdenied\x00 '+b'x'*2000)
  result=str(mailboxes.connection_error('POP3','password',c,error,password='private'))
  self.assertNotIn('\n',result);self.assertNotIn('\x1b',result);self.assertNotIn('\x00',result);self.assertLess(len(result),850);self.assertIn('Server reply:',result)
 def test_oversized_replies_are_not_partially_exposed(self):
  c=self.config('pop3');result=str(mailboxes.connection_error('POP3','password',c,mailboxes.poplib.error_proto(b'x'*65537),password='private'))
  self.assertIn('omitted because it is too large',result)
 def test_worker_json_preserves_sanitized_reason(self):
  import io
  request={'operation':'mail_connect','config':self.config('pop3'),'password':'private-password'};client=Mock();client.pass_.side_effect=mailboxes.poplib.error_proto(b'-ERR [AUTH] Account disabled private-password')
  stdin=Mock();stdin.buffer=io.BytesIO(json.dumps(request).encode());stdout=io.StringIO()
  with patch('worker.sys.stdin',stdin),patch('worker.sys.stdout',stdout),patch('mailboxes.poplib.POP3_SSL',return_value=client):code=worker.main()
  event=json.loads(stdout.getvalue());self.assertEqual(code,1);self.assertEqual(event['type'],'error');self.assertIn('[AUTH] Account disabled',event['message']);self.assertNotIn('private-password',stdout.getvalue())

if __name__=='__main__':unittest.main()
