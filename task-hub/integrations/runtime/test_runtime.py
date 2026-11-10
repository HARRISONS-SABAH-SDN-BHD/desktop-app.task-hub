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
if __name__=='__main__':unittest.main()
