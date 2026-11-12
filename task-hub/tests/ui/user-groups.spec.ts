import { test, expect } from "@playwright/test";
import { seedRecipients } from "./recipient-selection";
test("create edit delete and apply recipient groups to To or CC without changing existing routines", async ({
  page,
}) => {
  await seedRecipients(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Recipients", exact: true }).click();
  await page.getByRole("button", { name: "Create group", exact: true }).click();
  await page.getByLabel("Group name", { exact: true }).fill("Report team");
  const members = page.getByRole("group", { name: "Group members" });
  await members
    .getByRole("checkbox", { name: "manager@example.com", exact: true })
    .check();
  await members
    .getByRole("checkbox", { name: "team@example.com", exact: true })
    .check();
  await page.getByRole("button", { name: "Save group", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Edit group Report team", exact: true }),
  ).toBeVisible();
  await page.reload();
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await page.getByRole("button", { name: "New email routine" }).click();
  await page.getByLabel("Routine name", { exact: true }).fill("Grouped report");
  await page.getByLabel("First date", { exact: true }).fill("2099-01-01");
  await page
    .getByRole("button", { name: "Choose Recipient", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Choose recipients",
    exact: true,
  });
  await dialog
    .getByRole("button", { name: "Add Report team to To", exact: true })
    .click();
  await dialog
    .getByRole("button", { name: "Add Report team to To", exact: true })
    .click();
  await expect(
    dialog.getByRole("checkbox", {
      name: "To manager@example.com",
      exact: true,
    }),
  ).toBeChecked();
  await dialog
    .getByRole("button", { name: "Add Report team to CC", exact: true })
    .click();
  await expect(
    dialog.getByRole("checkbox", {
      name: "To manager@example.com",
      exact: true,
    }),
  ).not.toBeChecked();
  await expect(
    dialog.getByRole("checkbox", { name: "CC team@example.com", exact: true }),
  ).toBeChecked();
  await dialog
    .getByRole("checkbox", { name: "To older@example.com", exact: true })
    .check();
  await dialog
    .getByRole("button", { name: "Apply recipients", exact: true })
    .click();
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  let settings = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!)[0]
        .settings,
  );
  expect(settings.recipient).toBe("older@example.com");
  expect(
    settings.cc
      .split(",")
      .map((s: string) => s.trim())
      .sort(),
  ).toEqual(["manager@example.com", "team@example.com"]);
  await page.getByRole("button", { name: "Recipients", exact: true }).click();
  await page
    .getByRole("button", { name: "Edit group Report team", exact: true })
    .click();
  await members
    .getByRole("checkbox", { name: "team@example.com", exact: true })
    .uncheck();
  await page.getByRole("button", { name: "Save group", exact: true }).click();
  await page
    .getByRole("button", { name: "Delete group Report team", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Edit group Report team", exact: true }),
  ).toHaveCount(0);
  settings = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!)[0]
        .settings,
  );
  expect(settings.cc).toContain("team@example.com");
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
