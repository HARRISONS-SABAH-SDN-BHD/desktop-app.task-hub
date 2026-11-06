import { createElement } from "react";
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { GmailConnection } from "../src/components/GmailConnection";

describe("publisher-configured Google sign-in", () => {
  it("shows account sign-in without developer fields or Cloud instructions", () => {
    const html = renderToStaticMarkup(
      createElement(GmailConnection, {
        status: {
          publisher_configured: true,
          connected: false,
          email: null,
          client_id: "",
          busy: false,
        },
        native: true,
        busy: false,
        refresh: async () => {},
      }),
    );
    expect(html).toContain("Connect Gmail");
    expect(html).not.toContain("Desktop OAuth client ID");
    expect(html).not.toContain("Publisher setup instructions");
    expect(html).not.toContain("Developer connection settings");
    expect(html).not.toMatch(/type="submit" disabled/);
  });
  it("keeps unconfigured builds honest instead of offering broken sign-in", () => {
    const html = renderToStaticMarkup(
      createElement(GmailConnection, {
        status: { connected: false, email: null, client_id: "", busy: false },
        native: true,
        busy: false,
        refresh: async () => {},
      }),
    );
    expect(html).toContain("publisher needs to finish Google sign-in setup");
    expect(html).toContain("Developer connection settings");
    expect(html).toMatch(/type="submit" disabled/);
  });
});
