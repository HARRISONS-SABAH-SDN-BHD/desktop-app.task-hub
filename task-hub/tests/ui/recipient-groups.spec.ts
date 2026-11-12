import { test, expect } from "@playwright/test";
test("saved recipients separate providers, preserve each account filter, and label unknown legacy scans", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() =>
    localStorage.setItem(
      "taskhub.recipients.preview.v1",
      JSON.stringify({
        recipients: [
          {
            email: "first@example.com",
            name: "",
            source: "gmail",
            accounts: [{ provider: "gmail", email: "one@gmail.com" }],
          },
          {
            email: "second@example.com",
            name: "",
            source: "gmail",
            accounts: [{ provider: "gmail", email: "two@gmail.com" }],
          },
          {
            email: "shared@example.com",
            name: "",
            source: "gmail",
            accounts: [
              { provider: "gmail", email: "one@gmail.com" },
              { provider: "mailserver", email: "one@work.com" },
            ],
          },
          {
            email: "imap-first@example.com",
            name: "",
            source: "mailserver",
            accounts: [{ provider: "mailserver", email: "one@work.com" }],
          },
          {
            email: "imap-second@example.com",
            name: "",
            source: "mailserver",
            accounts: [{ provider: "mailserver", email: "two@work.com" }],
          },
          { email: "legacy@example.com", name: "", source: "gmail" },
          { email: "manual@example.com", name: "", source: "manual" },
        ],
        hidden: ["first@example.com"],
      }),
    ),
  );
  await page.reload();
  await page.getByRole("button", { name: "Recipients", exact: true }).click();
  await expect(
    page.getByRole("tab", { name: "Gmail", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  const list = page.getByRole("tabpanel");
  await expect(list.locator(".recipient-card")).toHaveCount(5);
  await page
    .getByLabel("Gmail recipient account", { exact: true })
    .selectOption("one@gmail.com");
  await expect(list.locator(".recipient-card")).toHaveCount(3);
  await expect(list).not.toContainText("second@example.com");
  await expect(
    list.getByText("shared@example.com", { exact: true }),
  ).toBeVisible();
  expect(
    await list
      .locator(".recipient-card small")
      .first()
      .evaluate((el) => getComputedStyle(el).display),
  ).toBe("block");
  await page
    .getByLabel("Gmail recipient account", { exact: true })
    .selectOption("two@gmail.com");
  await expect(list.locator(".recipient-card")).toHaveCount(2);
  await page.getByRole("tab", { name: "IMAP / POP3", exact: true }).click();
  await expect(list.locator(".recipient-card")).toHaveCount(4);
  await page
    .getByLabel("IMAP / POP3 recipient account", { exact: true })
    .selectOption("one@work.com");
  await expect(list.locator(".recipient-card")).toHaveCount(3);
  await expect(list).not.toContainText("imap-second@example.com");
  await page.getByRole("tab", { name: "Gmail", exact: true }).click();
  await expect(
    page.getByLabel("Gmail recipient account", { exact: true }),
  ).toHaveValue("two@gmail.com");
  await page
    .getByLabel("Gmail recipient account", { exact: true })
    .selectOption("unknown");
  await expect(list.locator(".recipient-card")).toHaveCount(2);
  await expect(list).toContainText("legacy@example.com");
  await page.getByRole("tab", { name: "IMAP / POP3", exact: true }).click();
  await expect(
    page.getByLabel("IMAP / POP3 recipient account", { exact: true }),
  ).toHaveValue("one@work.com");
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
