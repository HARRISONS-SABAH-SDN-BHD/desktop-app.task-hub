import { recipientAddresses, type Recipient } from "./recipients";
export interface RecipientUserGroup {
  id: string;
  name: string;
  members: string[];
}
export function groupFitsSender(
  group: RecipientUserGroup,
  contacts: Recipient[],
): boolean {
  return (
    group.members.length > 0 &&
    group.members.every((email) =>
      contacts.some((c) => c.email.toLowerCase() === email.toLowerCase()),
    )
  );
}
export function addGroupAddresses(
  current: { recipient: string[]; cc: string[] },
  members: string[],
  role: "recipient" | "cc",
) {
  const selected = [...new Set(members.map((s) => s.toLowerCase()))];
  const moved = new Set(selected);
  const next = {
    recipient: current.recipient.filter((s) => !moved.has(s.toLowerCase())),
    cc: current.cc.filter((s) => !moved.has(s.toLowerCase())),
  };
  next[role] = [...next[role], ...selected];
  recipientAddresses(next[role].join(", "));
  return next;
}
