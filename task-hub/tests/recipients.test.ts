import { describe, it, expect } from "vitest";
import {
  parseRecipientText,
  recipientAddresses,
  validateRecipientFile,
} from "../src/domain/recipients";
describe("recipient imports", () => {
  it("reads quoted CSV names, BOM and email headers; deduplicates addresses", () => {
    expect(
      parseRecipientText(
        '\uFEFFName,Email\r\n"Doe, Jane",jane@example.com\r\nJane,JANE@example.com\r\nDan,dan@example.com',
        true,
      ),
    ).toEqual(["jane@example.com", "dan@example.com"]);
    expect(parseRecipientText("a@example.com\n\nb@example.com", false)).toEqual(
      ["a@example.com", "b@example.com"],
    );
  });
  it("rejects invalid rows, ambiguous columns, empty files and header injection", () => {
    for (const text of [
      "Email\na@example.com\ninvalid",
      "Email,Recipient\na@example.com,b@example.com",
      "Email\n",
      '"a@example.com\nBcc: b@example.com"',
      '"broken',
    ])
      expect(() => parseRecipientText(text, true)).toThrow();
    expect(() => recipientAddresses("a@example.com, A@example.com")).toThrow();
    expect(() =>
      recipientAddresses("a@example.com\r\nBcc: b@example.com"),
    ).toThrow();
    expect(() =>
      recipientAddresses(
        Array.from({ length: 101 }, (_, i) => `a${i}@example.com`).join(","),
      ),
    ).toThrow();
  });
  it("validates only nonempty selections that belong to the imported list", () => {
    const file = {
      name: "team.csv",
      recipients: ["a@example.com", "b@example.com"],
    };
    expect(() => validateRecipientFile(file, "b@example.com")).not.toThrow();
    expect(() => validateRecipientFile(file, "other@example.com")).toThrow();
    expect(() => validateRecipientFile(file, "")).toThrow();
  });
});
