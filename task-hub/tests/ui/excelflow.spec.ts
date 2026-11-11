import { test, expect } from "@playwright/test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve, sep } from "node:path";

test("ExcelFlow workspace hands chosen reports to an existing routine", async ({
  page,
}) => {
  const root = await mkdtemp(join(tmpdir(), "taskhub-excelflow-"));
  try {
    const folder = join(root, "completed-run");
    const file = "Formatted October 2026/FTG/01 KK FTG 1026.xlsx";
    await mkdir(join(folder, "Formatted October 2026", "FTG"), {
      recursive: true,
    });
    await writeFile(join(folder, ...file.split("/")), "preview workbook");
    await writeFile(
      join(folder, "run_summary.json"),
      JSON.stringify({
        status: "complete",
        period: "2026-10",
        files: [{ file, kind: "FTG", branch: "A01", rows: 3 }],
      }),
    );
    await page.goto("/");
    await page.evaluate(() =>
      localStorage.setItem(
        "taskhub.email-routines.preview.v1",
        JSON.stringify([
          {
            id: "existing-target",
            settings: {
              name: "Existing monthly email",
              recipient: "team@example.com",
              sender_account: "preview@example.com",
              subject: "Report",
              message: "Report",
              start_date: "2099-01-01",
              time: "09:00",
              repeat: "monthly",
              weekdays: [0],
              enabled: true,
              auto_send: false,
              attachments: [],
              report_rules: [],
              calendar: { enabled: false, reminder_minutes: 60 },
              recipient_change: null,
            },
            next_due: "2099-01-01T01:00:00Z",
            attempt: null,
            notified_due: null,
          },
        ]),
      ),
    );
    await page.reload();
    await page.getByRole("button", { name: "ExcelFlow", exact: true }).click();
    await page
      .getByRole("button", { name: "DISC & FTG Reports", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Open results", exact: true })
      .click();
    await page
      .getByLabel("Email routine", { exact: true })
      .selectOption("existing-target");
    await page
      .getByLabel("ExcelFlow run folder", { exact: true })
      .setInputFiles(folder);
    await page.locator(".excelflow-report").getByRole("checkbox").check();
    await page
      .getByRole("button", { name: "Add selected reports", exact: true })
      .click();
    await expect(page.getByLabel("Routine name", { exact: true })).toHaveValue(
      "Existing monthly email",
    );
    await expect(
      page.getByRole("button", {
        name: "Remove 01 KK FTG 1026.xlsx",
        exact: true,
      }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Save routine" }).click();
    const stored = await page.evaluate(() =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!),
    );
    expect(stored).toHaveLength(1);
    expect(stored[0].id).toBe("existing-target");
    expect(stored[0].settings.attachments[0].name).toBe("01 KK FTG 1026.xlsx");
    await page.getByRole("button", { name: "ExcelFlow", exact: true }).click();
    await page.setViewportSize({ width: 390, height: 844 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
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
