import { expect, test } from "@playwright/test";
test("Gmail setup explains desktop OAuth and keeps real sign-in disabled in browser preview", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Connect", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("region", { name: "Gmail account connection" }),
  ).toContainText("Windows may be locked");
  await page.getByText("Publisher setup instructions", { exact: true }).click();
  await expect(page.getByText("Desktop app", { exact: true })).toBeVisible();
  await expect(page.getByLabel("Desktop OAuth client ID")).toHaveCount(0);
  await page
    .getByRole("button", { name: "Developer connection settings" })
    .click();
  await expect(page.getByLabel("Desktop OAuth client ID")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Connect Gmail", exact: true }),
  ).toBeDisabled();
  await page
    .getByLabel("Desktop OAuth client ID")
    .fill("123-test.apps.googleusercontent.com");
  await page
    .getByLabel("Desktop client secret (if provided)")
    .fill("test-secret-only");
  const storage = await page.evaluate(() =>
    JSON.stringify({ ...localStorage }),
  );
  expect(storage).not.toContain("test-secret-only");
});
test("Gmail connection instructions fit a narrow screen", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await page
    .getByRole("button", { name: "Developer connection settings" })
    .click();
  await expect(page.getByLabel("Desktop OAuth client ID")).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
