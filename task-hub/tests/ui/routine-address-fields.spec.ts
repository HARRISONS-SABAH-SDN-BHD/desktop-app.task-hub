import { test, expect } from "@playwright/test";
test("routine To CC BCC and recipient changes offer only sender-specific saved addresses and preserve typed values", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(() => {
    localStorage.setItem(
      "taskhub.recipients.preview.v1",
      JSON.stringify({
        recipients: [
          {
            email: "google-contact@example.com",
            name: "Google contact",
            source: "gmail",
            accounts: [{ provider: "gmail", email: "sender@gmail.com" }],
          },
          {
            email: "other-google@example.com",
            name: "Other Google",
            source: "gmail",
            accounts: [{ provider: "gmail", email: "other@gmail.com" }],
          },
          {
            email: "work-contact@example.com",
            name: "Work contact",
            source: "mailserver",
            accounts: [{ provider: "mailserver", email: "work@example.com" }],
          },
          {
            email: "shared-manual@example.com",
            name: "Manual",
            source: "manual",
          },
        ],
      }),
    );
    localStorage.setItem(
      "taskhub.email-routines.preview.v1",
      JSON.stringify([
        {
          id: "address-test",
          settings: {
            name: "Address fields",
            provider: "gmail",
            sender_account: "sender@gmail.com",
            recipient: "",
            cc: "",
            bcc: "",
            subject: "Report",
            message: "Body",
            start_date: "2099-01-01",
            time: "09:00",
            repeat: "monthly",
            weekdays: [],
            enabled: true,
            auto_send: false,
            attachments: [],
            recipient_change: null,
          },
          next_due: "2099-01-01T01:00:00Z",
          attempt: null,
          notified_due: null,
        },
      ]),
    );
  });
  await page.reload();
  await page
    .getByRole("button", { name: "Edit routine / recipient", exact: true })
    .click();
  const to = page.getByLabel("Choose saved recipient", { exact: true });
  const cc = page.getByLabel("Choose saved cc (optional)", { exact: true });
  const bcc = page.getByLabel("Choose saved bcc (optional)", { exact: true });
  for (const select of [to, cc, bcc]) {
    await expect(
      select.locator('option[value="google-contact@example.com"]'),
    ).toHaveCount(1);
    await expect(
      select.locator('option[value="other-google@example.com"]'),
    ).toHaveCount(0);
    await expect(
      select.locator('option[value="work-contact@example.com"]'),
    ).toHaveCount(0);
  }
  await to.selectOption("google-contact@example.com");
  await cc.selectOption("shared-manual@example.com");
  await cc.selectOption("google-contact@example.com");
  await expect(page.getByLabel("CC (optional)", { exact: true })).toHaveValue(
    /shared-manual@example\.com,\s*google-contact@example\.com/,
  );
  await bcc.selectOption("shared-manual@example.com");
  await page
    .getByLabel("BCC (optional)", { exact: true })
    .fill("hidden-typed@example.com");
  await page
    .getByLabel("Recipient change", { exact: true })
    .selectOption("temporary");
  await expect(
    page
      .getByLabel("Choose saved replacement recipient", { exact: true })
      .locator('option[value="work-contact@example.com"]'),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Remove recipient change", exact: true })
    .click();
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  await page.reload();
  await page
    .getByRole("button", { name: "Preview next email", exact: true })
    .click();
  const preview = page.getByRole("region", { name: "Routine email preview" });
  await expect(preview).toContainText("hidden-typed@example.com");
  await page
    .getByRole("button", { name: "Email routines", exact: true })
    .click();
  await page.evaluate(() => {
    const key = "taskhub.email-routines.preview.v1";
    const rows = JSON.parse(localStorage.getItem(key)!);
    rows[0].settings.provider = "mailserver";
    rows[0].settings.sender_account = "work@example.com";
    localStorage.setItem(key, JSON.stringify(rows));
  });
  await page.reload();
  await page
    .getByRole("button", { name: "Edit routine / recipient", exact: true })
    .click();
  for (const select of [to, cc, bcc]) {
    await expect(
      select.locator('option[value="work-contact@example.com"]'),
    ).toHaveCount(1);
    await expect(
      select.locator('option[value="google-contact@example.com"]'),
    ).toHaveCount(0);
  }
  await expect(page.getByLabel("BCC (optional)", { exact: true })).toHaveValue(
    "hidden-typed@example.com",
  );
  await page
    .getByLabel("CC (optional)", { exact: true })
    .fill("manual-copy@example.com");
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
