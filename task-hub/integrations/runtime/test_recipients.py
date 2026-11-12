import unittest
from unittest.mock import Mock,patch
import mailboxes,worker
class RecipientTests(unittest.TestCase):
    def config(self):
        return dict(email='me@example.com',protocol='pop3',host='mail.example.com',port=995,username='me',folder='INBOX',draft_folder='Drafts',smtp_host='',smtp_port=587,smtp_username='',smtp_security='starttls')
    def test_header_addresses_include_to_cc_and_reply_to_but_never_body_content(self):
        raw=b'From: Alice <alice@example.com>\r\nTo: me@example.com\r\nCc: Copy <copy@example.com>\r\nReply-To: reply@example.com\r\n\r\nContact body-only@example.com'
        contacts=mailboxes.message_info(raw,'1')['contacts']
        self.assertEqual([c['email'] for c in contacts],['alice@example.com','me@example.com','copy@example.com','reply@example.com'])
        self.assertNotIn('body-only@example.com',str(contacts))
    def test_scan_uses_pop_header_reads_without_downloading_or_deleting_mail(self):
        client=Mock();client.uidl.return_value=(b'OK',[b'1 stable-id'],10);client.top.return_value=(b'OK',[b'From: Alice <alice@example.com>',b'To: me@example.com'],10)
        with patch('mailboxes.poplib.POP3_SSL',return_value=client):
            result=worker.dispatch({'operation':'mail_contacts','config':self.config(),'password':'test'})
        self.assertEqual(result[0]['email'],'alice@example.com');client.retr.assert_not_called();client.dele.assert_not_called()
    def test_smtp_envelope_contains_selected_to_and_cc_once(self):
        client=Mock();client.sendmail.return_value={}
        with patch('mailboxes.smtp',return_value=client):
            result=mailboxes.deliver({'email':'sender@example.com'},'test',b'raw',['one@example.com','two@example.com'],['ONE@example.com','cc@example.com'],True)
        self.assertTrue(result['accepted']);client.sendmail.assert_called_once();self.assertEqual([a.lower() for a in client.sendmail.call_args.args[1]],['one@example.com','two@example.com','cc@example.com'])
    def test_bcc_is_envelope_only_on_smtp_and_attachment_bytes_are_preserved(self):
        from email.message import EmailMessage
        from email.parser import BytesParser
        from email import policy
        message=EmailMessage();message['From']='sender@example.com';message['To']='to@example.com';message['Cc']='cc@example.com';message['Bcc']='hidden@example.com, private@example.com';message.set_content('Body')
        data=bytes(range(256));message.add_attachment(data,maintype='application',subtype='octet-stream',filename='report.bin')
        client=Mock();client.sendmail.return_value={}
        with patch('mailboxes.smtp',return_value=client):
            worker.dispatch({'operation':'mail_deliver','config':self.config(),'password':'test','raw':__import__('base64').b64encode(message.as_bytes()).decode(),'recipient':['to@example.com'],'cc':['cc@example.com'],'bcc':['hidden@example.com','private@example.com','TO@example.com'],'send':True})
        client.sendmail.assert_called_once();envelope=client.sendmail.call_args.args[1];wire=client.sendmail.call_args.args[2]
        self.assertEqual([a.lower() for a in envelope],['to@example.com','cc@example.com'])
        parsed=BytesParser(policy=policy.default).parsebytes(wire)
        self.assertIsNone(parsed.get('Bcc'));self.assertNotIn(b'hidden@example.com',wire);self.assertNotIn(b'private@example.com',wire)
        self.assertEqual(next(parsed.iter_attachments()).get_payload(decode=True),data)
if __name__=='__main__':unittest.main()
