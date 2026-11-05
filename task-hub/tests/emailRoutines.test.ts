import { describe, expect, it } from "vitest";
import {
  newRoutine,
  validateCc,
  previewNextDue,
  previewRoutine,
  renderTemplate,
  type EmailRoutine,
} from "../src/domain/emailRoutines";

describe("email routine browser preview", () => {
  it("defaults to drafts and resolves direct delivery only after automatic sending is enabled", () => {
    const settings = newRoutine();
    expect(settings.auto_send).toBe(false);
    const routine: EmailRoutine = {
      id: "r",
      settings,
      attempt: null,
      notified_due: null,
      next_due: new Date(2099, 0, 1, 9).toISOString(),
    };
    expect(previewRoutine(routine).parameters.delivery).toBe("Draft");
    routine.settings.auto_send = true;
    expect(previewRoutine(routine).parameters.delivery).toBe("Send directly");
  });
  it("renders last month's year correctly across January without changing blank lines", () => {
    expect(
      renderTemplate(
        "{{previous_month}} {{previous_year}}\n\n{{month}} {{year}}",
        new Date(2027, 0, 5),
      ),
    ).toBe("December 2026\n\nJanuary 2027");
    expect(() => renderTemplate("{{unrecognised}}", new Date())).toThrow(
      "Unknown template field",
    );
  });
  it("keeps monthly day 31 after February and handles one-time completion", () => {
    const settings = {
      ...newRoutine(),
      start_date: "2026-01-31",
      time: "09:00",
    };
    const feb = new Date(previewNextDue(settings, new Date(2026, 0, 31, 10))!);
    expect(feb.getMonth()).toBe(1);
    expect(feb.getDate()).toBe(28);
    const march = new Date(
      previewNextDue(settings, new Date(2026, 1, 28, 10))!,
    );
    expect(march.getDate()).toBe(31);
    expect(
      previewNextDue({ ...settings, repeat: "once" }, new Date(2026, 1, 1)),
    ).toBeNull();
  });
  it("uses selected weekdays", () => {
    const settings = {
      ...newRoutine(),
      start_date: "2026-02-01",
      repeat: "weekly" as const,
      weekdays: [0, 4],
    };
    expect(new Date(previewNextDue(settings)!).getDay()).toBe(1);
  });
  it("applies recipient changes only to scheduled dates inside their effective range", () => {
    const routine: EmailRoutine = {
      id: "r",
      attempt: null,
      notified_due: null,
      next_due: new Date(2026, 10, 15, 9).toISOString(),
      settings: {
        ...newRoutine(),
        recipient: "original@example.com",
        recipient_change: {
          recipient: "cover@example.com",
          from: "2026-11-01",
          until: "2026-11-30",
          note: "Covering leave",
        },
      },
    };
    expect(previewRoutine(routine).parameters.recipient).toBe(
      "cover@example.com",
    );
    routine.next_due = new Date(2026, 11, 15, 9).toISOString();
    expect(previewRoutine(routine).parameters.delivery).toBe("Draft");
    expect(previewRoutine(routine).parameters.recipient).toBe(
      "original@example.com",
    );
    routine.settings.recipient_change!.until = null;
    expect(previewRoutine(routine).parameters.recipient).toBe(
      "cover@example.com",
    );
  });
});

it("accepts optional comma-separated CC addresses and rejects malformed lists or injected headers", () => {
  expect(() => validateCc("")).not.toThrow();
  expect(() => validateCc("one@example.com, two@example.com")).not.toThrow();
  for (const value of [
    "invalid",
    "one@example.com,",
    "one@example.com;two@example.com",
    "one@example.com\r\nBcc: hidden@example.com",
  ]) {
    expect(() => validateCc(value)).toThrow();
  }
  const settings = newRoutine();
  settings.cc = "copy@example.com";
  const routine: EmailRoutine = {
    id: "cc",
    settings,
    next_due: "2026-11-05T09:00:00Z",
    attempt: null,
    notified_due: null,
  };
  expect(previewRoutine(routine).parameters.cc).toBe(settings.cc);
  delete settings.cc;
  expect(previewRoutine(routine).parameters.cc).toBe("");
});
