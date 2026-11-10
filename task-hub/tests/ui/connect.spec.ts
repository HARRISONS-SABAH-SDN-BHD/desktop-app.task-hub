import { test, expect } from "@playwright/test";
test("Connect offers only Gmail and IMAP POP3 without storing passwords", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  const options = page.getByRole("group", {
    name: "Connection type",
    exact: true,
  });
  await expect(options.getByRole("button")).toHaveCount(2);
  await expect(
    options.getByRole("button", { name: "Gmail", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Microsoft 365|Outlook Classic/ }),
  ).toHaveCount(0);
  await options
    .getByRole("button", { name: "IMAP / POP3", exact: true })
    .click();
  await page
    .getByLabel("Email address", { exact: true })
    .fill("user@example.com");
  await page
    .getByLabel("Password or app password", { exact: true })
    .fill("secret-preview-never-save");
  await expect(
    page.getByRole("button", { name: "Connect mail server", exact: true }),
  ).toBeDisabled();
  await page
    .getByLabel("Incoming protocol", { exact: true })
    .selectOption("pop3");
  await expect(
    page.getByLabel("Incoming TLS port", { exact: true }),
  ).toHaveValue("995");
  expect(
    await page.evaluate(() => JSON.stringify({ ...localStorage })),
  ).not.toContain("secret-preview-never-save");
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
test("ExcelFlow preserves the original panel positions and collapses its sidebar", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "ExcelFlow", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Harrison ExcelFlow", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Expand sidebar" }),
  ).toHaveAttribute("aria-expanded", "false");
  await page
    .getByRole("button", { name: "DISC & FTG Reports", exact: true })
    .click();
  const workbooks = await page
    .getByRole("region", { name: "Workbooks", exact: true })
    .boundingBox();
  const details = await page
    .getByRole("region", { name: "Report details", exact: true })
    .boundingBox();
  const output = await page
    .getByRole("region", { name: "Output location", exact: true })
    .boundingBox();
  expect(Math.abs(workbooks!.y - details!.y)).toBeLessThan(2);
  expect(details!.x).toBeGreaterThan(workbooks!.x + workbooks!.width);
  expect(output!.y).toBeGreaterThan(workbooks!.y + workbooks!.height);
  await page.getByRole("button", { name: "Expand sidebar" }).click();
  await expect(
    page.getByRole("button", { name: "Collapse sidebar" }),
  ).toHaveAttribute("aria-expanded", "true");
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page
    .getByRole("checkbox", {
      name: "Detect month and year from filenames (YYMM)",
      exact: true,
    })
    .uncheck();
  await page
    .getByRole("dialog", { name: "Report settings" })
    .getByRole("button", { name: "Done", exact: true })
    .click();
  await expect(page.getByLabel("Report month", { exact: true })).toBeEnabled();
  await page
    .getByRole("button", { name: "Read from email", exact: true })
    .click();
  const source = page.getByRole("dialog", { name: "Workbook email source" });
  await expect(
    source.getByLabel("Email connection", { exact: true }),
  ).toHaveValue("gmail");
  await expect(
    source.getByLabel("Email connection", { exact: true }).getByRole("option"),
  ).toHaveText(["Gmail", "IMAP / POP3"]);
  await page.keyboard.press("Escape");
  await expect(source).toHaveCount(0);
  await page
    .getByRole("button", { name: "View run details", exact: true })
    .click();
  await expect(page.getByRole("tab", { name: "Date preview" })).toBeVisible();
  await page.getByRole("tab", { name: "Run log" }).click();
  await expect(page.getByRole("tabpanel")).toContainText(
    "No run has started yet.",
  );
  await page
    .getByRole("button", { name: "Back to report", exact: true })
    .click();
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
