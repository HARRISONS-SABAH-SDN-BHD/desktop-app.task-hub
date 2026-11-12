import { expect, test } from "@playwright/test";

test("calendar sync is optional, persists its reminder, and remains preview-only in a browser", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "New email routine" }).click();
  await page
    .getByLabel("Routine name", { exact: true })
    .fill("Calendar report");
  await page.getByLabel("Recipient", { exact: true }).fill("team@example.com");
  const sync = page.getByRole("checkbox", { name: "Sync to Google Calendar" });
  await expect(sync).not.toBeChecked();
  await sync.check();
  await page
    .getByLabel("Calendar reminder", { exact: true })
    .selectOption("60");
  await page
    .getByRole("button", { name: "Help: Calendar reminders", exact: true })
    .click();
  await expect(
    page.getByText(/recipients and files are not copied to the calendar/),
  ).toBeVisible();
  await page.getByRole("button", { name: "Save routine" }).click();
  await expect(
    page.getByRole("heading", { name: "Calendar report", exact: true }),
  ).toBeVisible();
  await page.reload();
  await page.getByText("Routine details", { exact: true }).click();
  await expect(
    page.getByText("Google Calendar: Preview only — no event created", {
      exact: true,
    }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Edit routine / recipient" }).click();
  await expect(sync).toBeChecked();
  await expect(
    page.getByLabel("Calendar reminder", { exact: true }),
  ).toHaveValue("60");
  await sync.uncheck();
  await page.getByRole("button", { name: "Save routine" }).click();
  await expect(
    page.getByText("Google Calendar: Preview only — no event created", {
      exact: true,
    }),
  ).toHaveCount(0);
});

test("calendar access is a separate opt-in on Google sign-in and background operation is explained", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  const calendar = page.getByRole("checkbox", {
    name: "Enable Google Calendar access",
  });
  await expect(calendar).not.toBeChecked();
  await calendar.check();
  await expect(calendar).toBeChecked();
  await expect(
    page.getByRole("button", { name: "Connect Gmail", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("region", { name: "Gmail account connection" }),
  ).toContainText("system tray");
  await page.getByText("Publisher setup instructions", { exact: true }).click();
  await expect(
    page.getByRole("button", {
      name: "Enable Google Calendar API",
      exact: true,
    }),
  ).toBeVisible();
});
