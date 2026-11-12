export const MAX_RECIPIENTS = 100;
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

export type RecipientProvider = "gmail" | "mailserver";
export interface RecipientAccount {
  provider: RecipientProvider;
  email: string;
}
export interface Recipient {
  email: string;
  name: string;
  source: "manual" | "gmail" | "mailserver";
  accounts?: RecipientAccount[];
}
