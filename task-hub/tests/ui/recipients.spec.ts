import { test, expect } from "@playwright/test";
test("recipient directory supports add, deduplication, search, deletion and routine selection", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Recipients", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Scan emails", exact: true }),
  ).toBeDisabled();
  await page.getByLabel("Recipient name", { exact: true }).fill("Manager");
  await page
    .getByLabel("Recipient email address", { exact: true })
    .fill("manager@example.com");
  await page
    .getByRole("button", { name: "Add recipient", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Saved recipients (1)" }),
  ).toBeVisible();
  await page
    .getByLabel("Recipient email address", { exact: true })
    .fill("MANAGER@example.com");
  await page
    .getByRole("button", { name: "Add recipient", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Saved recipients (1)" }),
  ).toBeVisible();
  await page.getByLabel("Recipient name", { exact: true }).fill("Cover");
  await page
    .getByLabel("Recipient email address", { exact: true })
    .fill("cover@example.com");
  await page
    .getByRole("button", { name: "Add recipient", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Saved recipients (2)" }),
  ).toBeVisible();
  await page.reload();
  await page.getByRole("button", { name: "Recipients", exact: true }).click();
  await page.getByLabel("Search saved recipients").fill("cover");
  await expect(
    page.getByRole("button", {
      name: "Delete manager@example.com",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Delete cover@example.com", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await page.getByRole("button", { name: "New email routine" }).click();
  await page.getByLabel("Routine name", { exact: true }).fill("New routine");
  await expect(page.getByLabel("Recipient file", { exact: true })).toHaveCount(
    0,
  );
  await expect(
    page.getByRole("region", { name: "Email history suggestions" }),
  ).toHaveCount(0);
  await page
    .getByLabel("Choose saved recipient", { exact: true })
    .selectOption("manager@example.com");
  await expect(page.getByLabel("Recipient", { exact: true })).toHaveValue(
    "manager@example.com",
  );
  await page
    .getByLabel("Recipient change", { exact: true })
    .selectOption("temporary");
  await page
    .getByLabel("Choose saved replacement recipient", { exact: true })
    .selectOption("cover@example.com");
  await page.getByLabel("Effective from", { exact: true }).fill("2099-01-01");
  await page
    .getByLabel("Through (inclusive)", { exact: true })
    .fill("2099-01-07");
  await page.getByLabel("First date", { exact: true }).fill("2099-01-01");
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  await page
    .getByRole("button", { name: "Preview next email", exact: true })
    .click();
  await expect(
    page.getByRole("region", { name: "Routine email preview" }),
  ).toContainText("cover@example.com");
  await page.getByRole("button", { name: "Recipients", exact: true }).click();
  await page
    .getByRole("button", { name: "Delete cover@example.com", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Saved recipients (1)" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Edit routine / recipient", exact: true })
    .click();
  await expect(
    page.getByLabel("Replacement recipient", { exact: true }),
  ).toHaveValue("cover@example.com");
  await page
    .getByRole("button", { name: "Remove recipient change", exact: true })
    .click();
  await expect(
    page.getByLabel("Replacement recipient", { exact: true }),
  ).toHaveCount(0);
  await page.getByLabel("Recipient", { exact: true }).fill("typed@example.com");
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  let settings = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!)[0]
        .settings,
  );
  expect(settings.recipient_change).toBeNull();
  expect(settings.recipient).toBe("typed@example.com");
  expect(settings.recipient_file).toBeUndefined();
  await page
    .getByRole("button", { name: "Edit routine / recipient", exact: true })
    .click();
  await page
    .getByLabel("Recipient change", { exact: true })
    .selectOption("permanent");
  await page
    .getByLabel("Replacement recipient", { exact: true })
    .fill("new@example.com");
  await page.getByLabel("Effective from", { exact: true }).fill("2099-01-01");
  await expect(
    page.getByLabel("Through (inclusive)", { exact: true }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  await page.reload();
  await page
    .getByRole("button", { name: "Preview next email", exact: true })
    .click();
  await expect(
    page.getByRole("region", { name: "Routine email preview" }),
  ).toContainText("new@example.com");
  await page.getByRole("button", { name: "Recipients", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
test("temporary recipient dates must be valid and removed legacy file metadata cannot restrict typing", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "New email routine" }).click();
  await page.getByLabel("Routine name", { exact: true }).fill("Dates");
  await page.getByLabel("Recipient", { exact: true }).fill("main@example.com");
  await page
    .getByLabel("Recipient change", { exact: true })
    .selectOption("temporary");
  await page
    .getByLabel("Replacement recipient", { exact: true })
    .fill("cover@example.com");
  await page.getByLabel("Effective from", { exact: true }).fill("2099-01-07");
  await page
    .getByLabel("Through (inclusive)", { exact: true })
    .fill("2099-01-01");
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  expect(
    await page
      .getByLabel("Through (inclusive)", { exact: true })
      .evaluate((input: HTMLInputElement) => input.validity.rangeUnderflow),
  ).toBe(true);
  await page
    .getByRole("button", { name: "Remove recipient change", exact: true })
    .click();
  await page.getByLabel("First date", { exact: true }).fill("2099-01-01");
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  await page.evaluate(() => {
    const key = "taskhub.email-routines.preview.v1";
    const rows = JSON.parse(localStorage.getItem(key)!);
    rows[0].settings.recipient_file = {
      name: "old.csv",
      recipients: ["main@example.com"],
    };
    localStorage.setItem(key, JSON.stringify(rows));
  });
  await page.reload();
  await page
    .getByRole("button", { name: "Edit routine / recipient", exact: true })
    .click();
  await page
    .getByLabel("Recipient", { exact: true })
    .fill("changed@example.com");
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  const saved = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!)[0]
        .settings,
  );
  expect(saved.recipient).toBe("changed@example.com");
  expect(saved.recipient_file).toBeUndefined();
});
