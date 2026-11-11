import { test, expect } from "@playwright/test";
test("recipient file choices persist, search does not lose selections, invalid replacement keeps the valid list", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "New email routine" }).click();
  await page
    .getByLabel("Routine name", { exact: true })
    .fill("Recipient selection test");
  const input = page.getByLabel("Recipient file", { exact: true });
  await input.setInputFiles({
    name: "team.csv",
    mimeType: "text/csv",
    buffer: Buffer.from(
      "Name,Email\nOne,one@example.com\nTwo,two@example.com\nDuplicate,ONE@example.com\nThree,three@example.com",
    ),
  });
  await expect(
    page.getByText("3 of 3 selected", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("checkbox", { name: "two@example.com", exact: true })
    .uncheck();
  await page.getByLabel("Search recipients").fill("three");
  await expect(
    page.getByRole("checkbox", { name: "one@example.com", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByText("2 of 3 selected", { exact: true }),
  ).toBeVisible();
  await input.setInputFiles({
    name: "bad.csv",
    mimeType: "text/csv",
    buffer: Buffer.from("Email\ninvalid"),
  });
  await expect(page.getByRole("alert")).toContainText("row 2");
  await expect(
    page.getByText("2 of 3 selected", { exact: true }),
  ).toBeVisible();
  await page.getByLabel("Search recipients").fill("");
  await page
    .getByRole("button", { name: "Clear selection", exact: true })
    .click();
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Select 1–100");
  await page.getByRole("button", { name: "Select all", exact: true }).click();
  await page
    .getByRole("checkbox", { name: "two@example.com", exact: true })
    .uncheck();
  await page.getByLabel("First date", { exact: true }).fill("2099-01-01");
  await expect(
    page.getByRole("combobox", { name: "Recipient change", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("region", { name: "ExcelFlow report import" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  await page.reload();
  await page
    .getByRole("button", { name: "Edit routine / recipient", exact: true })
    .click();
  await expect(
    page.getByRole("checkbox", { name: "one@example.com", exact: true }),
  ).toBeChecked();
  await expect(
    page.getByRole("checkbox", { name: "two@example.com", exact: true }),
  ).not.toBeChecked();
  await expect(
    page.getByRole("checkbox", { name: "three@example.com", exact: true }),
  ).toBeChecked();
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  const settings = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!)[0]
        .settings,
  );
  expect(settings.recipient).toBe("one@example.com, three@example.com");
  expect(settings.attachments).toEqual([]);
  expect(settings.report_rules).toEqual([]);
  expect(settings.recipient_change).toBeNull();
  expect(settings.recipient_file.name).toBe("team.csv");
  await page
    .getByRole("button", { name: "Preview next email", exact: true })
    .click();
  const preview = page.getByRole("region", { name: "Routine email preview" });
  await expect(preview).toContainText("one@example.com, three@example.com");
  await expect(preview).not.toContainText("two@example.com");
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
