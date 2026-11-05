import { test, expect } from "@playwright/test";
test("update controls are isolated from email routines and disabled in browser preview", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "App updates", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "App updates", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Check for updates", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Install and restart", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("region", { name: "Task Hub updates" }),
  ).toContainText("backs up local routines and history");
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "New email routine", exact: true }),
  ).toBeVisible();
});
