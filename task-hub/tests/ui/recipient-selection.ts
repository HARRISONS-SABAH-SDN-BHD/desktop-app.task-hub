import { expect, type Page } from "@playwright/test";
export async function seedRecipients(page: Page) {
  await page.addInitScript(() => {
    const key = "taskhub.recipients.preview.v1";
    if (!localStorage.getItem(key))
      localStorage.setItem(
        key,
        JSON.stringify({
          recipients: [
            "team@example.com",
            "manager@example.com",
            "older@example.com",
            "copy@example.com",
          ].map((email) => ({ email, name: "", source: "manual" })),
        }),
      );
  });
}
export async function chooseRecipients(
  page: Page,
  values: { To?: string; CC?: string; BCC?: string },
) {
  await page
    .getByRole("button", { name: "Choose Recipient", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Choose recipients",
    exact: true,
  });
  for (const [role, value] of Object.entries(values)) {
    for (const checkbox of await dialog
      .getByRole("checkbox", { name: new RegExp("^" + role + " ") })
      .all())
      await checkbox.uncheck();
    for (const email of value
      .split(",")
      .map((v) => v.trim())
      .filter(Boolean))
      await dialog
        .getByRole("checkbox", { name: role + " " + email, exact: true })
        .check();
  }
  await dialog
    .getByRole("button", { name: "Apply recipients", exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
}
