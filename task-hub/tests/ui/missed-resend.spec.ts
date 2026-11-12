import { test, expect } from "@playwright/test";
test("Resend now is offered only for missed unattempted automatic routines and never sends from browser preview", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() => {
    const settings = {
      provider: "gmail",
      name: "Missed",
      recipient: "to@example.com",
      sender_account: "preview@example.com",
      subject: "Report",
      message: "Body",
      start_date: "2020-01-01",
      time: "09:00",
      repeat: "daily",
      weekdays: [],
      enabled: true,
      auto_send: true,
      attachments: [],
      recipient_change: null,
    };
    const routines = [
      {
        id: "missed",
        settings,
        next_due: "2020-01-01T01:00:00Z",
        attempt: null,
        notified_due: null,
      },
      {
        id: "attempted",
        settings: { ...settings, name: "Already attempted" },
        next_due: "2020-01-01T01:00:00Z",
        attempt: "attempt-id",
        notified_due: null,
      },
      {
        id: "paused",
        settings: { ...settings, name: "Paused", enabled: false },
        next_due: "2020-01-01T01:00:00Z",
        attempt: null,
        notified_due: null,
      },
      {
        id: "draft",
        settings: { ...settings, name: "Draft", auto_send: false },
        next_due: "2020-01-01T01:00:00Z",
        attempt: null,
        notified_due: null,
      },
      {
        id: "future",
        settings: { ...settings, name: "Future" },
        next_due: "2099-01-01T01:00:00Z",
        attempt: null,
        notified_due: null,
      },
    ];
    localStorage.setItem(
      "taskhub.email-routines.preview.v1",
      JSON.stringify(routines),
    );
  });
  await page.reload();
  const button = page.getByRole("button", { name: "Resend now", exact: true });
  await expect(button).toHaveCount(1);
  await expect(button).toBeDisabled();
  await expect(
    page
      .locator(".routine-card")
      .filter({
        has: page.getByRole("heading", { name: "Missed", exact: true }),
      }),
  ).toContainText("send this occurrence immediately");
});
