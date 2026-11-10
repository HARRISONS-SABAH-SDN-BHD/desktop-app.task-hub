import { test, expect } from "@playwright/test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve, sep } from "node:path";
test("ExcelFlow completed run selection adds only chosen reports and persists snapshots", async ({
  page,
}) => {
  const root = await mkdtemp(join(tmpdir(), "taskhub-excelflow-"));
  try {
    const folder = join(root, "DISC & FTG Reports test");
    const file = "Formatted October 2026/DISC/01 KK DISC 1026.xlsx";
    await mkdir(join(folder, "Formatted October 2026", "DISC"), {
      recursive: true,
    });
    await writeFile(join(folder, ...file.split("/")), "browser preview report");
    await writeFile(
      join(folder, "run_summary.json"),
      JSON.stringify({
        status: "complete",
        period: "2026-10",
        files: [{ file, kind: "DISC", branch: "A01", rows: 12 }],
      }),
    );
    await page.goto("/");
    await page.getByRole("button", { name: "New email routine" }).click();
    await page
      .getByLabel("Routine name", { exact: true })
      .fill("ExcelFlow monthly report");
    await page
      .getByLabel("Default recipient", { exact: true })
      .fill("team@example.com");
    await page.getByLabel("First date", { exact: true }).fill("2099-01-01");
    await page
      .getByLabel("ExcelFlow run folder", { exact: true })
      .setInputFiles(folder);
    await expect(
      page.getByText("Report month: 2026-10", { exact: true }),
    ).toBeVisible();
    const report = page
      .locator(".excelflow-report")
      .filter({ hasText: "01 KK DISC 1026.xlsx" });
    await expect(report).toContainText("A01 · DISC · Normal · 12 rows");
    await report.getByRole("checkbox").check();
    await page
      .getByRole("button", { name: "Add selected reports", exact: true })
      .click();
    await expect(
      page.getByRole("button", {
        name: "Remove 01 KK DISC 1026.xlsx",
        exact: true,
      }),
    ).toBeVisible();
    await report.getByRole("checkbox").check();
    await page
      .getByRole("button", { name: "Add selected reports", exact: true })
      .click();
    await expect(page.getByRole("alert")).toContainText("share filenames");
    await page.setViewportSize({ width: 390, height: 844 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.getByRole("button", { name: "Save routine" }).click();
    await page.reload();
    await page.getByText("Routine details", { exact: true }).click();
    await expect(
      page.getByText("Attachments: 01 KK DISC 1026.xlsx", { exact: true }),
    ).toBeVisible();
    const stored = await page.evaluate(
      () =>
        JSON.parse(
          localStorage.getItem("taskhub.email-routines.preview.v1")!,
        )[0].settings.attachments,
    );
    expect(stored).toHaveLength(1);
    expect(stored[0].name).not.toContain("json");
  } finally {
    const checked = resolve(root);
    if (
      !checked.startsWith(resolve(tmpdir()) + sep) ||
      !checked.split(sep).pop()!.startsWith("taskhub-excelflow-")
    )
      throw new Error("Unsafe test cleanup path");
    await rm(checked, { recursive: true, force: true });
  }
});
