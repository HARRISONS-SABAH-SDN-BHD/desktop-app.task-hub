import { expect, test } from "@playwright/test";
test("saved routine retains attachment details after reload and uses them in the due preview", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await page.getByRole("button", { name: "New email routine" }).click();
  await page
    .getByLabel("Routine name", { exact: true })
    .fill("Report with file");
  await page
    .getByLabel("Default recipient", { exact: true })
    .fill("manager@example.com");
  await page.getByLabel("First date", { exact: true }).fill("2020-01-01");
  await page
    .getByLabel("CC (optional)", { exact: true })
    .fill("copy@example.com, team@example.com");
  await page.getByLabel("Attachment files").setInputFiles({
    name: "monthly.csv",
    mimeType: "text/csv",
    buffer: Buffer.from("month,total\nJanuary,10"),
  });
  await expect(
    page.getByRole("button", { name: "Remove monthly.csv" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Save routine" }).click();
  await page.reload();
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await expect(
    page.getByText("Attachments: monthly.csv", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText(/^CC: copy@example\.com,\s*team@example\.com$/),
  ).toBeVisible();
  await page.getByRole("button", { name: "Preview due email" }).click();
  await expect(
    page.getByRole("region", { name: "Routine email preview" }),
  ).toContainText(/copy@example\.com,\s*team@example\.com/);
  await expect(
    page.getByRole("region", { name: "Routine email preview" }),
  ).toContainText("monthly.csv");
  await page.getByRole("button", { name: "Preview routine only" }).click();
  await expect(page.getByLabel("Execution log")).toContainText("monthly.csv");
});
