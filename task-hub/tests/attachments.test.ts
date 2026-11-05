import { describe, expect, it } from "vitest";
import {
  MAX_ATTACHMENT_BYTES,
  mergeAttachments,
  validateAttachments,
  type AttachmentRef,
} from "../src/domain/attachments";
const file = (id: string, name = "report.pdf", size = 100): AttachmentRef => ({
  id,
  name,
  size,
  sha256: "a".repeat(64),
});
describe("attachment selections", () => {
  it("does not add the same file twice and keeps the original list unchanged", () => {
    const original = [file("one")];
    expect(mergeAttachments(original, [file("two")])).toEqual(original);
    expect(
      mergeAttachments(original, [file("three", "notes.txt")]),
    ).toHaveLength(2);
    expect(original).toHaveLength(1);
  });
  it("enforces combined size and file count before saving", () => {
    expect(() =>
      validateAttachments([file("one", "large.zip", MAX_ATTACHMENT_BYTES + 1)]),
    ).toThrow("18 MB");
    expect(() =>
      validateAttachments([
        file("one", "one.zip", MAX_ATTACHMENT_BYTES),
        file("two", "extra.txt", 1),
      ]),
    ).toThrow("18 MB");
    expect(() =>
      validateAttachments(
        Array.from({ length: 11 }, (_, i) => file(String(i))),
      ),
    ).toThrow("10 files");
    expect(() => validateAttachments([file("same"), file("same")])).toThrow(
      "Duplicate",
    );
  });
});
