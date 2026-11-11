import { expect, test } from "@playwright/test";
test("Unsupported provider routine is preserved but must choose a supported sender", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() =>
    localStorage.setItem(
      "taskhub.email-routines.preview.v1",
      JSON.stringify([
        {
          id: "ms-test",
          settings: {
            provider: "retired-provider",
            name: "Work report",
            recipient: "team@example.com",
            sender_account: "work@example.com",
            subject: "Report",
            message: "Test report only",
            start_date: "2099-01-01",
            time: "09:00",
            repeat: "monthly",
            weekdays: [0],
            enabled: true,
            auto_send: false,
            cc: "copy@example.com",
            attachments: [],
            report_rules: [],
            calendar: { enabled: false, reminder_minutes: 60 },
            recipient_change: null,
          },
          next_due: "2099-01-01T01:00:00Z",
          attempt: null,
          notified_due: null,
        },
      ]),
    ),
  );
  await page.reload();
  await page.getByRole("button", { name: "Edit routine / recipient" }).click();
  await expect(page.getByLabel("Sender account", { exact: true })).toHaveValue(
    "retired-provider:work@example.com",
  );
  await expect(
    page.getByRole("button", { name: "Preview next email" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Save routine" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "This provider is unsupported",
  );
  await page
    .getByLabel("Sender account", { exact: true })
    .selectOption("gmail:preview@example.com");
  await page.getByRole("button", { name: "Save routine" }).click();
  await page.reload();
  const saved = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!)[0],
  );
  expect(saved.settings.provider).toBe("gmail");
  expect(saved.settings.recipient).toBe("team@example.com");
  expect(saved.settings.message).toBe("Test report only");
  expect(saved.settings.sender_account).toBe("preview@example.com");
});
