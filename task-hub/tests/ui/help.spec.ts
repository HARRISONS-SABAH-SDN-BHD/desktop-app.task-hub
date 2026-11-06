import { test, expect } from "@playwright/test";
test("help stays collapsed until requested and supports keyboard dismissal", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "New email routine" }).click();
  const help = page.getByRole("button", {
    name: "Help: Choosing attachments",
    exact: true,
  });
  await expect(help).toHaveAttribute("aria-expanded", "false");
  await help.click();
  await expect(help).toHaveAttribute("aria-expanded", "true");
  await expect(
    page.getByText("Use a saved copy for files that stay the same.", {
      exact: false,
    }),
  ).toBeVisible();
  await help.press("Escape");
  await expect(help).toHaveAttribute("aria-expanded", "false");
  await expect(help).toBeFocused();
  await help.click();
  await page.getByLabel("Routine name", { exact: true }).click();
  await expect(help).toHaveAttribute("aria-expanded", "false");
});
