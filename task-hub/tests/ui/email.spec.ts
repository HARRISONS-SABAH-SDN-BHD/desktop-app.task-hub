import { test, expect } from "@playwright/test";
import legacy from "../../config/examples/library.tasks.json" with { type: "json" };

test("old browser storage cannot reintroduce non-email tasks or history", async ({
  page,
}) => {
  await page.addInitScript((config) => {
    const makeRun = (id: string, task_id: string, title: string) => ({
      id,
      task_id,
      title,
      status: "succeeded",
      started_at: "2026-10-20T00:00:00Z",
      parameters: {},
      logs: [],
    });
    localStorage.setItem(
      "taskhub.preview.v1",
      JSON.stringify({
        config,
        foreground_migrated: true,
        snapshot: {
          runs: [
            makeRun("old-word", "foreground_word", "Old Word run"),
            makeRun("old-email", "foreground_email", "Previous email"),
          ],
          schedules: [],
          output_dir: "test",
        },
      }),
    );
  }, legacy);
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Email routines", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Email history", exact: true })
    .click();
  await expect(page.locator(".history-row")).toHaveCount(1);
  await expect(page.locator(".history-row")).toContainText("Previous email");
  await expect(page.getByText("Old Word run")).toHaveCount(0);
});

test("routines are the home page and the one-off automation tab is removed", async ({
  page,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Email routines", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Email automation", exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Email history", exact: true })
    .click();
  await page.getByRole("link", { name: /taskhub/ }).click();
  await expect(
    page.getByRole("heading", { name: "Email routines", exact: true }),
  ).toBeVisible();
});
