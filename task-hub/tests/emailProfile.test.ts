import { describe, it, expect } from "vitest";
import { emailConfig, validateEmailRequest } from "../src/domain/emailProfile";
import {
  defaults,
  resolveFields,
  validateParameters,
} from "../src/domain/config";

describe("email MVP scope", () => {
  it("exposes exactly one email task with fixed Gmail and Chrome integration", () => {
    const config = emailConfig();
    expect(config.tasks).toHaveLength(1);
    const task = config.tasks[0];
    const values = {
      ...defaults(task),
      recipient: "team@example.com",
      subject: "Update",
      message: "Hello",
    };
    validateParameters(task, values);
    expect(resolveFields(task, values)).toMatchObject({
      provider: "Gmail",
      browser: "Google Chrome",
      delivery: "Draft",
    });
    expect(task.steps.find((s) => s.id === "delivery")?.options).toEqual([
      "Draft",
      "Send after review",
    ]);
  });
  it("blocks legacy tasks and direct sends at the product boundary", () => {
    expect(() => validateEmailRequest("foreground_word", {})).toThrow(
      "email automation only",
    );
    expect(() =>
      validateEmailRequest("foreground_email", { delivery: "Send directly" }),
    ).toThrow("Draft or Send after review");
    expect(() =>
      validateEmailRequest("foreground_email", {
        delivery: "Send after review",
      }),
    ).not.toThrow();
  });
});
