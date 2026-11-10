import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
export interface GmailStatus {
  calendar_access?: boolean;
  read_access?: boolean;
  publisher_configured?: boolean;
  connected: boolean;
  email: string | null;
  client_id: string;
  busy: boolean;
}
export const gmailConnection = {
  async status(): Promise<GmailStatus> {
    return bridge.native
      ? invoke("gmail_status")
      : { connected: false, email: null, client_id: "", busy: false };
  },
  async connect(
    clientId: string,
    clientSecret: string,
    calendarAccess = false,
    readAccess = false,
  ): Promise<void> {
    if (!bridge.native)
      throw new Error(
        "Google sign-in is available in the desktop app. Browser preview does not connect an account.",
      );
    return invoke("connect_gmail", {
      clientId,
      calendarAccess,
      readAccess,
      clientSecret: clientSecret || null,
    });
  },
  async cancel(): Promise<void> {
    if (bridge.native) return invoke("cancel_gmail_connection");
  },
  async disconnect(): Promise<void> {
    if (bridge.native) return invoke("disconnect_gmail");
  },
  async openSetup(which: "cloud" | "oauth" | "calendar"): Promise<void> {
    if (bridge.native) return invoke("open_gmail_setup", { which });
    window.open(
      which === "calendar"
        ? "https://console.cloud.google.com/apis/library/calendar-json.googleapis.com"
        : which === "cloud"
          ? "https://console.cloud.google.com/apis/library/gmail.googleapis.com"
          : "https://developers.google.com/identity/protocols/oauth2/native-app",
      "_blank",
      "noopener,noreferrer",
    );
  },
};
