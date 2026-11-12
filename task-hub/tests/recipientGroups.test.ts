import { describe, it, expect } from "vitest";
import {
  routineRecipients,
  filterRecipientGroup,
  recipientAccountChoices,
  hasUnknownAccount,
  UNKNOWN_ACCOUNT,
} from "../src/domain/recipientGroups";
import type { Recipient } from "../src/domain/recipients";
const contacts: Recipient[] = [
  {
    email: "shared@example.com",
    name: "Shared",
    source: "gmail",
    accounts: [
      { provider: "gmail", email: "one@gmail.com" },
      { provider: "gmail", email: "two@gmail.com" },
      { provider: "mailserver", email: "work@example.com" },
    ],
  },
  { email: "legacy@example.com", name: "", source: "gmail" },
  { email: "manual@example.com", name: "Manual", source: "manual" },
];
describe("recipient provider and account groups", () => {
  it("keeps all recorded memberships without duplicating contacts", () => {
    expect(
      recipientAccountChoices(contacts, "gmail", "three@gmail.com"),
    ).toEqual(["one@gmail.com", "three@gmail.com", "two@gmail.com"]);
    expect(
      filterRecipientGroup(contacts, "gmail", "one@gmail.com").map(
        (c) => c.email,
      ),
    ).toEqual(["shared@example.com", "manual@example.com"]);
    expect(
      filterRecipientGroup(contacts, "mailserver", "work@example.com").map(
        (c) => c.email,
      ),
    ).toEqual(["shared@example.com", "manual@example.com"]);
  });
  it("does not guess which mailbox older contacts came from", () => {
    expect(hasUnknownAccount(contacts[1], "gmail")).toBe(true);
    expect(
      filterRecipientGroup(contacts, "gmail", UNKNOWN_ACCOUNT).map(
        (c) => c.email,
      ),
    ).toEqual(["legacy@example.com", "manual@example.com"]);
    expect(
      filterRecipientGroup(contacts, "mailserver", "all").some(
        (c) => c.email === "legacy@example.com",
      ),
    ).toBe(false);
  });
  it("keeps old provider evidence until rescanning identifies that provider account", () => {
    const contact: Recipient = {
      email: "old@example.com",
      name: "",
      source: "gmail",
      accounts: [{ provider: "mailserver", email: "work@example.com" }],
    };
    expect(hasUnknownAccount(contact, "gmail")).toBe(true);
    expect(
      filterRecipientGroup([contact], "gmail", UNKNOWN_ACCOUNT),
    ).toHaveLength(1);
  });
  it("routine choices match the exact sender mailbox plus manual contacts", () => {
    expect(
      routineRecipients(contacts, "gmail", "one@gmail.com").map((c) => c.email),
    ).toEqual(["shared@example.com", "manual@example.com"]);
    expect(
      routineRecipients(contacts, "gmail", "unscanned@gmail.com").map(
        (c) => c.email,
      ),
    ).toEqual(["manual@example.com"]);
    expect(
      routineRecipients(contacts, "mailserver", "work@example.com").map(
        (c) => c.email,
      ),
    ).toEqual(["shared@example.com", "manual@example.com"]);
    expect(
      routineRecipients(contacts, "mailserver", "different@example.com").map(
        (c) => c.email,
      ),
    ).toEqual(["manual@example.com"]);
  });
});
