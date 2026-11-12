import { expect, test } from "@playwright/test";

test("automatic sending is explicitly enabled, persisted, and not executed by browser preview", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await page.getByRole("button", { name: "New email routine" }).click();
  await page
    .getByLabel("Routine name", { exact: true })
    .fill("Scheduled email");
  await page.getByLabel("Recipient", { exact: true }).fill("team@example.com");
  await page.getByLabel("First date", { exact: true }).fill("2099-01-01");
  await expect(
    page.getByRole("checkbox", {
      name: "Automatically send when due",
      exact: true,
    }),
  ).not.toBeChecked();
  await page
    .getByRole("checkbox", { name: "Automatically send when due", exact: true })
    .check();
  await page
    .getByRole("button", { name: "Help: Automatic sending", exact: true })
    .click();
  await expect(
    page.getByText(/Saving authorizes Task Hub to send/),
  ).toBeVisible();
  await page.getByRole("button", { name: "Save routine" }).click();
  await page.reload();
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await expect(
    page.getByText("Automatic send · no approval required", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Preview next email" }).click();
  await expect(
    page.getByRole("region", { name: "Routine email preview" }),
  ).toContainText("Automatic sending is enabled");
  await expect(
    page.getByRole("button", { name: "Preview routine only" }),
  ).toHaveCount(0);
});

test("a due routine can prepare a draft directly without a review or send approval", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await page.getByRole("button", { name: "New email routine" }).click();
  await page.getByLabel("Routine name", { exact: true }).fill("One-time draft");
  await page.getByLabel("Recipient", { exact: true }).fill("team@example.com");
  await page.getByLabel("First date", { exact: true }).fill("2020-01-01");
  await page
    .getByRole("combobox", { name: "Repeat", exact: true })
    .selectOption("once");
  await page.getByRole("button", { name: "Save routine" }).click();
  await expect(
    page.getByRole("region", { name: "Routine email preview" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Preview routine only" }).click();
  await expect(page.getByLabel("Execution log")).toContainText("PREVIEW ONLY");
  const delivery = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.preview.v1")!).snapshot.runs[0]
        .parameters.delivery,
  );
  expect(delivery).toBe("Draft");
  await expect(
    page.getByRole("button", { name: "Approve sending this email" }),
  ).toHaveCount(0);
});

test("a saved due routine reuses templates, keeps selected recipients and persists", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await page.getByRole("button", { name: "New email routine" }).click();
  await page.getByLabel("Routine name", { exact: true }).fill("Monthly report");
  await page
    .getByLabel("Recipient", { exact: true })
    .fill("manager@example.com");
  await page.getByLabel("First date", { exact: true }).fill("2020-01-31");
  await page.getByRole("button", { name: "Save routine" }).click();
  await expect(
    page.getByText("Due for drafting", { exact: true }),
  ).toBeVisible();
  await page.reload();
  await page
    .getByRole("button", { name: "Email history", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText("due for drafting");
  await page.getByRole("button", { name: "View due routines" }).click();
  await page
    .getByRole("button", { name: "Preview due email", exact: true })
    .click();
  const review = page.getByRole("region", { name: "Routine email preview" });
  await expect(review).toContainText("manager@example.com");
  await expect(review).toContainText("December 2019 report");
  await page.getByRole("button", { name: "Preview routine only" }).click();
  await expect(page.getByLabel("Execution log")).toContainText("PREVIEW ONLY");
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await expect(page.getByText("Due for drafting", { exact: true })).toHaveCount(
    0,
  );
  await page.getByRole("button", { name: "Preview next email" }).click();
  await expect(
    page.getByRole("region", { name: "Routine email preview" }),
  ).toContainText("manager@example.com");
});

test("weekly routine requires selected days and fits a narrow screen", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await page.getByRole("button", { name: "New email routine" }).click();
  await page.getByLabel("Routine name", { exact: true }).fill("Weekly report");
  await page
    .getByLabel("Recipient", { exact: true })
    .fill("manager@example.com");
  await page
    .getByRole("combobox", { name: "Repeat", exact: true })
    .selectOption("weekly");
  await page.getByLabel("Mon", { exact: true }).uncheck();
  await page.getByRole("button", { name: "Save routine" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Choose at least one weekday",
  );
  await page.getByLabel("Fri", { exact: true }).check();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
