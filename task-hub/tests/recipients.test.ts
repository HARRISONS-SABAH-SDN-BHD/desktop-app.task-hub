import { describe, it, expect } from "vitest";
import { validRecipient, recipientAddresses } from "../src/domain/recipients";
describe("recipient address fields", () => {
  it("accepts typed single or multiple addresses", () => {
    expect(validRecipient("person@example.com")).toBe(true);
    expect(recipientAddresses("a@example.com, b@example.com")).toEqual([
      "a@example.com",
      "b@example.com",
    ]);
  });
  it("rejects empty selections, duplicate addresses and header injection", () => {
    for (const text of [
      "",
      "invalid",
      "a@example.com, A@example.com",
      "a@example.com\r\nBcc: b@example.com",
    ])
      expect(() => recipientAddresses(text)).toThrow();
    expect(() =>
      recipientAddresses(
        Array.from({ length: 101 }, (_, i) => `a${i}@example.com`).join(","),
      ),
    ).toThrow();
  });
});
