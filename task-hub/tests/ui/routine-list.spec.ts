import { expect, test } from "@playwright/test";

test("routine cards stay newest first after editing and support search and narrow screens", async ({
  page,
}) => {
  await page.goto("/");
  for (const name of ["Older report", "Newer report", "Latest report"]) {
    await page.getByRole("button", { name: "New email routine" }).click();
    await page.getByLabel("Routine name", { exact: true }).fill(name);
    await page
      .getByLabel("Default recipient", { exact: true })
      .fill(
        name.startsWith("Older") ? "older@example.com" : "team@example.com",
      );
    await page.getByLabel("First date", { exact: true }).fill("2099-01-01");
    await page.getByRole("button", { name: "Save routine" }).click();
  }
  const cards = page.locator(".routine-card");
  const headings = cards.getByRole("heading", { level: 2 });
  await expect(headings).toHaveText([
    "Latest report",
    "Newer report",
    "Older report",
  ]);
  const first = await cards.nth(0).boundingBox();
  const second = await cards.nth(1).boundingBox();
  expect(first!.y).toBe(second!.y);
  expect(second!.x).toBeGreaterThan(first!.x);
  await cards
    .nth(2)
    .getByRole("button", { name: "Edit routine / recipient" })
    .click();
  await page
    .getByLabel("Routine name", { exact: true })
    .fill("Older report edited");
  await page.getByRole("button", { name: "Save routine" }).click();
  await page.reload();
  await expect(headings).toHaveText([
    "Latest report",
    "Newer report",
    "Older report edited",
  ]);
  const search = page.getByRole("searchbox", { name: "Find a routine" });
  await search.fill("OLDER@EXAMPLE.COM");
  await expect(headings).toHaveText(["Older report edited"]);
  await search.fill("no match");
  await expect(cards).toHaveCount(0);
  await expect(page.getByText(/No matching routines/)).toBeVisible();
  await search.fill("");
  await cards.first().getByText("Routine details", { exact: true }).click();
  await expect(cards.first().getByText(/^Sender:/)).toBeVisible();
  await cards.first().getByText("More actions", { exact: true }).click();
  await expect(
    cards.first().getByRole("button", { name: "Delete routine" }),
  ).toBeVisible();
  await page.screenshot({
    path: "C:/Users/danie/Documents/Codex/2026-10-20/so/work/routine-cards-preview.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  const narrowFirst = await cards.nth(0).boundingBox();
  const narrowSecond = await cards.nth(1).boundingBox();
  expect(narrowSecond!.y).toBeGreaterThan(narrowFirst!.y + narrowFirst!.height);
});
