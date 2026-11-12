import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
import { validRecipient, type Recipient } from "../domain/recipients";
const key = "taskhub.recipients.preview.v1";
interface Book {
  recipients: Recipient[];
}
function read(): Book {
  return JSON.parse(localStorage.getItem(key) ?? '{"recipients":[]}') as Book;
}
export const recipientBook = {
  async list(): Promise<Recipient[]> {
    return bridge.native
      ? invoke("list_recipients")
      : read().recipients.sort((a, b) =>
          (a.name || a.email).localeCompare(b.name || b.email),
        );
  },
  async save(email: string, name: string): Promise<void> {
    email = email.trim().toLowerCase();
    name = name.trim();
    if (
      !validRecipient(email) ||
      name.length > 100 ||
      /[\x00-\x1f\x7f]/.test(name)
    )
      throw new Error(
        "Enter a valid email address and a name of up to 100 characters.",
      );
    if (bridge.native) return invoke("save_recipient", { email, name });
    const book = read();
    const index = book.recipients.findIndex((c) => c.email === email);
    const contact: Recipient = { email, name, source: "manual" };
    if (index >= 0)
      book.recipients[index] = {
        ...contact,
        accounts: book.recipients[index].accounts,
      };
    else {
      if (book.recipients.length >= 10000)
        throw new Error("The recipient list is full.");
      book.recipients.push(contact);
    }
    localStorage.setItem(key, JSON.stringify({ recipients: book.recipients }));
  },
  async remove(email: string): Promise<void> {
    if (bridge.native) return invoke("delete_recipient", { email });
    const book = read();
    book.recipients = book.recipients.filter((c) => c.email !== email);
    localStorage.setItem(key, JSON.stringify({ recipients: book.recipients }));
  },
  async scan(provider: string, sender: string): Promise<number> {
    if (!bridge.native)
      throw new Error("Email scanning is available in the desktop app.");
    return invoke("scan_recipients", { provider, sender });
  },
};
