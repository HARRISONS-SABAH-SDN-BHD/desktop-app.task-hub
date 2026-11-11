import { test, expect } from "@playwright/test";
test("report rules are removed from new routines and saving explicitly replaces legacy settings", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "New email routine" }).click();
  await expect(
    page.getByRole("button", { name: "Add current report rule" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("region", { name: "ExcelFlow report import" }),
  ).toHaveCount(0);
  await page.evaluate(() =>
    localStorage.setItem(
      "taskhub.email-routines.preview.v1",
      JSON.stringify([
        {
          id: "legacy",
          settings: {
            name: "Legacy report",
            recipient: "team@example.com",
            sender_account: "preview@example.com",
            subject: "Report",
            message: "Report",
            start_date: "2099-01-01",
            time: "09:00",
            repeat: "monthly",
            weekdays: [0],
            enabled: true,
            auto_send: false,
            attachments: [],
            report_rules: [
              {
                id: "00000000-0000-4000-8000-000000000001",
                folder: { id: "missing", path: "legacy" },
                pattern: "report.xlsx",
                previous_month: false,
                template: null,
                require_changed: true,
              },
            ],
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
  await expect(
    page.getByText(/Saving this routine removes its previous report rules/),
  ).toBeVisible();
  await page.getByRole("button", { name: "Save routine" }).click();
  const rules = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!)[0]
        .settings.report_rules,
  );
  expect(rules).toEqual([]);
});
