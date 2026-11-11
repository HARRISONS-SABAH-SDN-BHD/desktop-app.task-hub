export interface RecipientFile {
  name: string;
  recipients: string[];
}
export const MAX_RECIPIENTS = 100;
export const MAX_RECIPIENT_FILE = 2 * 1024 * 1024;
export function validRecipient(value: string): boolean {
  const parts = value.split("@");
  return (
    value.length <= 254 &&
    parts.length === 2 &&
    !!parts[0] &&
    parts[1].includes(".") &&
    !parts[1].startsWith(".") &&
    !parts[1].endsWith(".") &&
    !/[\s<>,;"\x00-\x1f\x7f]/.test(value)
  );
}
export function recipientAddresses(value: string): string[] {
  const addresses = value.split(",").map((v) => v.trim());
  if (
    value.length > 8000 ||
    /[\r\n\t]/.test(value) ||
    addresses.length > MAX_RECIPIENTS ||
    addresses.some((v) => !validRecipient(v)) ||
    new Set(addresses.map((v) => v.toLowerCase())).size !== addresses.length
  )
    throw new Error(
      "Select 1–100 distinct, valid recipient email addresses (up to 8,000 characters total).",
    );
  return addresses;
}
export function validateRecipientFile(
  file: RecipientFile | undefined,
  selected: string,
): void {
  const addresses = recipientAddresses(selected);
  if (!file) return; // Existing routines keep their saved recipients.
  if (
    !file.name ||
    file.name.length > 255 ||
    file.recipients.length > MAX_RECIPIENTS ||
    !file.recipients.length ||
    file.recipients.some((v) => !validRecipient(v)) ||
    new Set(file.recipients.map((v) => v.toLowerCase())).size !==
      file.recipients.length ||
    addresses.some((v) => !file.recipients.includes(v))
  )
    throw new Error(
      "Import a valid recipient list and select at least one address from it.",
    );
}
// CSV parsing keeps quoted commas and escaped quotes intact; it never extracts
// addresses from arbitrary prose or silently skips an invalid recipient row.
export function parseRecipientText(text: string, csv: boolean): string[] {
  if (new TextEncoder().encode(text).length > MAX_RECIPIENT_FILE)
    throw new Error("Recipient files must be 2 MB or smaller.");
  const rows: string[][] = [];
  if (!csv)
    rows.push(
      ...text
        .replace(/^\uFEFF/, "")
        .split(/\r?\n/)
        .map((v) => [v]),
    );
  else {
    let row: string[] = [],
      cell = "",
      quoted = false,
      closed = false;
    const input = text.replace(/^\uFEFF/, "").replace(/\r\n/g, "\n");
    for (let i = 0; i < input.length; i++) {
      const c = input[i];
      if (quoted) {
        if (c === '"') {
          if (input[i + 1] === '"') {
            cell += '"';
            i++;
          } else {
            quoted = false;
            closed = true;
          }
        } else cell += c;
      } else if (c === '"' && !cell && !closed) quoted = true;
      else if (c === "," || c === "\n") {
        row.push(cell);
        cell = "";
        closed = false;
        if (c === "\n") {
          rows.push(row);
          row = [];
        }
      } else {
        if (closed && c.trim()) throw new Error("Invalid CSV quoting.");
        if (!closed) cell += c;
      }
    }
    if (quoted) throw new Error("Invalid CSV quoting.");
    row.push(cell);
    rows.push(row);
  }
  return recipientsFromRows(rows);
}
export function recipientsFromRows(rows: string[][]): string[] {
  const nonblank = rows
    .map((cells, i) => ({ cells: cells.map((v) => v.trim()), line: i + 1 }))
    .filter((r) => r.cells.some(Boolean));
  if (!nonblank.length) throw new Error("The recipient file is empty.");
  const header = nonblank[0].cells.map((v) =>
    v.toLowerCase().replace(/[ _-]/g, ""),
  );
  const columns = header.flatMap((v, i) =>
    ["email", "emailaddress", "recipient", "recipients"].includes(v) ? [i] : [],
  );
  if (columns.length > 1) throw new Error("Use exactly one email column.");
  const column = columns[0] ?? 0;
  if (columns.length) nonblank.shift();
  else if (nonblank.some((r) => r.cells.length !== 1))
    throw new Error("Add an Email column header to a multi-column file.");
  const result: string[] = [],
    seen = new Set<string>();
  for (const row of nonblank) {
    const address = row.cells[column] ?? "";
    if (!validRecipient(address))
      throw new Error(
        `Invalid email address on row ${row.line}. Correct the file and import it again.`,
      );
    if (!seen.has(address.toLowerCase())) {
      result.push(address);
      seen.add(address.toLowerCase());
    }
  }
  recipientAddresses(result.join(", "));
  return result;
}
