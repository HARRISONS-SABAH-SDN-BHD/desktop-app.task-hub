import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
export interface MailConfig {
  email: string;
  protocol: "imap" | "pop3";
  host: string;
  port: number;
  incoming_security: "tls" | "starttls" | "none";
  username: string;
  folder: string;
  draft_folder: string;
  smtp_host: string;
  smtp_port: number;
  smtp_username: string;
  smtp_security: "tls" | "starttls";
}
export interface MailServerStatus {
  connected: boolean;
  can_send: boolean;
  config: MailConfig | null;
}
export const emptyMailServerStatus: MailServerStatus = {
  connected: false,
  can_send: false,
  config: null,
};
export const otherConnections = {
  async mailStatus(): Promise<MailServerStatus> {
    return bridge.native ? invoke("mailserver_status") : emptyMailServerStatus;
  },
  async classicStatus(): Promise<boolean> {
    return bridge.native ? invoke("outlook_classic_status") : false;
  },
  async connectMail(config: MailConfig, password: string): Promise<void> {
    if (!bridge.native)
      throw new Error("Use the desktop app to connect a mail server.");
    return invoke("connect_mailserver", { config, password });
  },
  async disconnectMail(): Promise<void> {
    return invoke("disconnect_mailserver");
  },
  async connectClassic(): Promise<{ connected: boolean; profile: string }> {
    if (!bridge.native)
      throw new Error("Outlook Classic requires the Windows desktop app.");
    return invoke("connect_outlook_classic");
  },
  async disconnectClassic(): Promise<void> {
    return invoke("disconnect_outlook_classic");
  },
};
