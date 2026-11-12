import type { Recipient, RecipientProvider } from "./recipients";
export const ALL_ACCOUNTS = "all";
export const UNKNOWN_ACCOUNT = "unknown";
export function recipientAccounts(
  contact: Recipient,
  provider: RecipientProvider,
): string[] {
  return [
    ...new Set(
      (contact.accounts ?? [])
        .filter((a) => a.provider === provider)
        .map((a) => a.email.toLowerCase()),
    ),
  ];
}
export function belongsToProvider(
  contact: Recipient,
  provider: RecipientProvider,
): boolean {
  return (
    contact.source === "manual" ||
    contact.source === provider ||
    recipientAccounts(contact, provider).length > 0
  );
}
export function recipientAccountChoices(
  contacts: Recipient[],
  provider: RecipientProvider,
  connected: string | null,
): string[] {
  return [
    ...new Set([
      ...(connected ? [connected.toLowerCase()] : []),
      ...contacts.flatMap((c) => recipientAccounts(c, provider)),
    ]),
  ].sort();
}
export function hasUnknownAccount(
  contact: Recipient,
  provider: RecipientProvider,
): boolean {
  return (
    contact.source === provider && !recipientAccounts(contact, provider).length
  );
}
export function filterRecipientGroup(
  contacts: Recipient[],
  provider: RecipientProvider,
  account: string,
): Recipient[] {
  return contacts.filter(
    (c) =>
      belongsToProvider(c, provider) &&
      (account === ALL_ACCOUNTS ||
        c.source === "manual" ||
        (account === UNKNOWN_ACCOUNT
          ? hasUnknownAccount(c, provider)
          : recipientAccounts(c, provider).includes(account.toLowerCase()))),
  );
}

export function routineRecipients(
  contacts: Recipient[],
  provider: string | undefined,
  sender: string | null,
): Recipient[] {
  if (!sender || (provider !== "gmail" && provider !== "mailserver"))
    return contacts.filter((c) => c.source === "manual");
  return filterRecipientGroup(contacts, provider, sender.toLowerCase());
}
