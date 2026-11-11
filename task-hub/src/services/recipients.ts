import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
import {
  MAX_RECIPIENT_FILE,
  parseRecipientText,
  validateRecipientFile,
  type RecipientFile,
} from "../domain/recipients";
export async function importRecipientFile(file: File): Promise<RecipientFile> {
  if (!/\.(csv|txt|xlsx)$/i.test(file.name))
    throw new Error("Choose a CSV, TXT or XLSX recipient file.");
  if (file.size > MAX_RECIPIENT_FILE)
    throw new Error("Recipient files must be 2 MB or smaller.");
  let recipients: string[];
  if (bridge.native) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    let binary = "";
    for (let i = 0; i < bytes.length; i += 8192)
      binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
    recipients = await invoke<string[]>("import_recipient_file", {
      name: file.name,
      data: btoa(binary),
    });
  } else {
    if (/\.xlsx$/i.test(file.name))
      throw new Error(
        "Excel recipient import is available in the desktop app. Use CSV or TXT in browser preview.",
      );
    recipients = parseRecipientText(
      await file.text(),
      /\.csv$/i.test(file.name),
    );
  }
  const result = { name: file.name, recipients };
  validateRecipientFile(result, recipients.join(", "));
  return result;
}
