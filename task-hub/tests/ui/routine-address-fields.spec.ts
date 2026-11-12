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

  await expect(page.getByLabel("Recipient", { exact: true })).toHaveCount(0);
  await expect(page.getByLabel("CC (optional)", { exact: true })).toHaveCount(
    0,
  );
  await page
    .getByRole("button", { name: "Choose Recipient", exact: true })
    .click();
  let dialog = page.getByRole("dialog", {
    name: "Choose recipients",
    exact: true,
  });
  await expect(
    dialog.getByRole("checkbox", {
      name: "To other-google@example.com",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    dialog.getByRole("checkbox", {
      name: "To work-contact@example.com",
      exact: true,
    }),
  ).toHaveCount(0);
  await dialog
    .getByRole("checkbox", {
      name: "To google-contact@example.com",
      exact: true,
    })
    .check();
  await dialog
    .getByRole("checkbox", {
      name: "BCC shared-manual@example.com",
      exact: true,
    })
    .check();
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByRole("region", { name: "Recipient selection", exact: true }),
  ).toContainText("No recipients selected");
  await page
    .getByRole("button", { name: "Choose Recipient", exact: true })
    .click();
  await dialog
    .getByRole("button", { name: "Apply recipients", exact: true })
    .click();
  await expect(dialog.getByRole("alert")).toContainText("at least one To");
  await dialog
    .getByRole("checkbox", {
      name: "To google-contact@example.com",
      exact: true,
    })
    .check();
  await dialog.getByLabel("Search recipient emails").fill("shared");
  await dialog
    .getByRole("checkbox", {
      name: "BCC shared-manual@example.com",
      exact: true,
    })
    .check();
  await dialog.getByLabel("Search recipient emails").fill("");
  await dialog
    .getByRole("button", { name: "Apply recipients", exact: true })
    .click();
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  await page.reload();
  await page
    .getByRole("button", { name: "Preview next email", exact: true })
    .click();
  await expect(
    page.getByRole("region", { name: "Routine email preview" }),
  ).toContainText("shared-manual@example.com");
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
  await page
    .getByRole("button", { name: "Choose Recipient", exact: true })
    .click();
  await expect(
    dialog.getByRole("checkbox", {
      name: "To work-contact@example.com",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    dialog.getByRole("checkbox", {
      name: "To other-google@example.com",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(dialog).toContainText("Previously selected address");
  await dialog
    .getByRole("checkbox", {
      name: "To google-contact@example.com",
      exact: true,
    })
    .uncheck();
  await dialog
    .getByRole("checkbox", { name: "To work-contact@example.com", exact: true })
    .check();
  await dialog
    .getByRole("checkbox", {
      name: "CC shared-manual@example.com",
      exact: true,
    })
    .check();
  await expect(
    dialog.getByRole("checkbox", {
      name: "BCC shared-manual@example.com",
      exact: true,
    }),
  ).not.toBeChecked();
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(
    true,
  );
  await dialog
    .getByRole("button", { name: "Apply recipients", exact: true })
    .click();
  await page.getByRole("button", { name: "Save routine", exact: true }).click();
  const saved = await page.evaluate(
    () =>
      JSON.parse(localStorage.getItem("taskhub.email-routines.preview.v1")!)[0]
        .settings,
  );
  expect(saved.recipient).toBe("work-contact@example.com");
  expect(saved.cc).toBe("shared-manual@example.com");
  expect(saved.bcc).toBe("");
});
