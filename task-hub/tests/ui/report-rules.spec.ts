import { expect, test } from "@playwright/test";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve, dirname, basename } from "node:path";
async function cleanupFixture(folder: string) {
  const target = resolve(folder),
    root = resolve(tmpdir());
  if (
    dirname(target) !== root ||
    !basename(target).startsWith("taskhub-reports-")
  )
    throw new Error("Unexpected fixture cleanup path");
  await rm(target, { recursive: true, force: true });
}
test("learns filename patterns, tests one occurrence and includes that period's report in browser preview", async ({
  page,
}) => {
  const folder = await mkdtemp(join(tmpdir(), "taskhub-reports-"));
  try {
    for (const name of ["4-11-2020.xlsx", "5-11-2020.xlsx"])
      await writeFile(
        join(folder, name),
        "SYNTHETIC_REPORT_CONTENT_NOT_FOR_STORAGE",
      );
    await page.goto("/");
    await page.getByRole("button", { name: "New email routine" }).click();
    await page
      .getByLabel("Routine name", { exact: true })
      .fill("Current monthly report");
    await page
      .getByLabel("Default recipient", { exact: true })
      .fill("team@example.com");
    await page.getByLabel("First date", { exact: true }).fill("2020-11-05");
    await page.getByRole("button", { name: "Add current report rule" }).click();
    await page
      .getByLabel("Report folder files 1", { exact: true })
      .setInputFiles(folder);
    await page
      .locator(".report-suggestions button")
      .filter({ hasText: "{{day}}-{{month_number}}-{{year}}.xlsx" })
      .click();
    await page
      .getByLabel("Test report date 1", { exact: true })
      .fill("2020-11-05");
    await page.getByRole("button", { name: "Test report rule" }).click();
    await expect(page.getByRole("status")).toContainText("One matching file");
    await expect(page.getByRole("status")).toContainText("5-11-2020.xlsx");
    await page.getByRole("button", { name: "Save routine" }).click();
    await expect(
      page.getByRole("heading", {
        name: "Current monthly report",
        exact: true,
      }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Preview due email" }).click();
    await page
      .getByRole("button", { name: "Preview routine only", exact: true })
      .click();
    await expect(page.getByLabel("Execution log")).toContainText(
      "5-11-2020.xlsx",
    );
    expect(
      await page.evaluate(() => JSON.stringify({ ...localStorage })),
    ).not.toContain("SYNTHETIC_REPORT_CONTENT_NOT_FOR_STORAGE");
  } finally {
    await cleanupFixture(folder);
  }
});
test("ambiguous filenames stop selection and a missing report explains template completion", async ({
  page,
}) => {
  const folder = await mkdtemp(join(tmpdir(), "taskhub-reports-"));
  try {
    for (const name of ["NOV draft.xlsx", "NOV final.xlsx"])
      await writeFile(join(folder, name), "Synthetic");
    await page.goto("/");
    await page.getByRole("button", { name: "New email routine" }).click();
    await page.getByRole("button", { name: "Add current report rule" }).click();
    await page
      .getByLabel("Report folder files 1", { exact: true })
      .setInputFiles(folder);
    await page
      .getByLabel("Filename pattern 1", { exact: true })
      .fill("NOV*.xlsx");
    await page.getByRole("button", { name: "Test report rule" }).click();
    await expect(page.getByRole("status")).toContainText("Ambiguous");
    await page
      .getByLabel("Filename pattern 1", { exact: true })
      .fill("Missing {{year}}.xlsx");
    await page
      .getByRole("region", { name: "Report template 1", exact: true })
      .getByLabel("Attachment files")
      .setInputFiles({
        name: "blank.xlsx",
        mimeType: "application/octet-stream",
        buffer: Buffer.from("Fill in actual figures"),
      });
    await expect(
      page.getByRole("button", { name: "Remove blank.xlsx" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Test report rule" }).click();
    await expect(page.getByRole("status")).toContainText(
      "create a template copy and stop for completion",
    );
    await page.setViewportSize({ width: 390, height: 844 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  } finally {
    await cleanupFixture(folder);
  }
});
