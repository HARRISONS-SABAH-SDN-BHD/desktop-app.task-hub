import base64,io,unittest
from unittest.mock import Mock,patch
import recipients,mailboxes,worker
class RecipientTests(unittest.TestCase):
    def read(self,text,name='team.csv'):
        return worker.dispatch({'operation':'read_recipients','name':name,'data':base64.b64encode(text.encode()).decode()})
    def test_csv_and_txt_headers_quotes_bom_and_deduplication(self):
        self.assertEqual(self.read('\ufeffName,Email\r\n"Doe, Jane",jane@example.com\r\nJane,JANE@example.com'),['jane@example.com'])
        self.assertEqual(self.read('a@example.com\n\nb@example.com','team.txt'),['a@example.com','b@example.com'])
    def test_invalid_lists_never_return_partial_recipients(self):
        for text in ['Email\na@example.com\ninvalid','Email,Recipient\na@example.com,b@example.com','Email\n','"a@example.com\nBcc: b@example.com"']:
            with self.subTest(text=text),self.assertRaises(ValueError):self.read(text)
        with self.assertRaises(ValueError):self.read('\n'.join(f'a{i}@example.com' for i in range(101)))
        with self.assertRaises(ValueError):self.read('a'*(2*1024*1024+1))
    def test_excel_import_reads_first_sheet_and_rejects_formula_recipient(self):
        from openpyxl import Workbook
        book=Workbook();book.active.append(['Name','Email']);book.active.append(['Dan','dan@example.com']);book.create_sheet().append(['ignored@example.com'])
        stream=io.BytesIO();book.save(stream)
        self.assertEqual(recipients.read('team.xlsx',base64.b64encode(stream.getvalue()).decode()),['dan@example.com'])
        book.active.append(['Formula','=A1']);stream=io.BytesIO();book.save(stream)
        with self.assertRaises(ValueError):recipients.read('team.xlsx',base64.b64encode(stream.getvalue()).decode())
    def test_smtp_envelope_contains_selected_to_and_cc_once(self):
        client=Mock();client.sendmail.return_value={}
        with patch('mailboxes.smtp',return_value=client):
            result=mailboxes.deliver({'email':'sender@example.com'},'test',b'raw',['one@example.com','two@example.com'],['ONE@example.com','cc@example.com'],True)
        self.assertTrue(result['accepted']);client.sendmail.assert_called_once()
        self.assertEqual([a.lower() for a in client.sendmail.call_args.args[1]],['one@example.com','two@example.com','cc@example.com'])
if __name__=='__main__':unittest.main()
