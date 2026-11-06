import { expect, test } from "@playwright/test";
test("Microsoft connection explains IT setup and stays disabled in browser preview", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Microsoft 365", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Microsoft 365 connection" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Connect Microsoft 365", exact: true }),
  ).toBeDisabled();
  await page
    .getByText("Microsoft setup instructions for IT", { exact: true })
    .click();
  await expect(page.getByText(/Allow public client flows/)).toBeVisible();
  await expect(page.getByLabel("Application (client) ID")).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
test("Microsoft routine keeps provider and sender through edit, reload and preview", async ({
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
            provider: "microsoft",
            name: "Work report",
            recipient: "team@example.com",
            sender_account: "work@harrisons.com.my",
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
    "microsoft:work@harrisons.com.my",
  );
  await expect(
    page.getByRole("checkbox", { name: "Sync to Google Calendar" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Save routine" }).click();
  await page.reload();
  const saved = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!)[0],
  );
  expect(saved.settings.provider).toBe("microsoft");
  expect(saved.settings.sender_account).toBe("work@harrisons.com.my");
  await page.getByRole("button", { name: "Preview next email" }).click();
  await expect(
    page.getByRole("region", { name: "Routine email preview" }),
  ).toContainText("work@harrisons.com.my");
});
