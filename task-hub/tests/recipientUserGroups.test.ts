import { it, expect } from "vitest";
import {
  addGroupAddresses,
  groupFitsSender,
} from "../src/domain/recipientUserGroups";
it("expands groups without duplicate delivery roles", () => {
  expect(
    addGroupAddresses(
      { recipient: ["other@example.com"], cc: ["A@example.com"] },
      ["a@example.com", "A@example.com", "b@example.com"],
      "recipient",
    ),
  ).toEqual({
    recipient: ["other@example.com", "a@example.com", "b@example.com"],
    cc: [],
  });
});
it("does not silently use only part of a group for a sender", () => {
  expect(
    groupFitsSender(
      { id: "1", name: "Team", members: ["a@example.com", "b@example.com"] },
      [{ email: "a@example.com", name: "", source: "manual" }],
    ),
  ).toBe(false);
});
