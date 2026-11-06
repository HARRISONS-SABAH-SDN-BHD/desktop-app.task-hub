import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
export interface MicrosoftStatus {
  calendar_access?: boolean;
  connected: boolean;
  email: string | null;
  client_id: string;
  tenant_id: string;
  publisher_configured: boolean;
  user_code: string | null;
  busy: boolean;
}
export const emptyMicrosoftStatus: MicrosoftStatus = {
  connected: false,
  email: null,
  client_id: "",
  tenant_id: "",
  publisher_configured: false,
  user_code: null,
  busy: false,
};
export const microsoftConnection = {
  async status(): Promise<MicrosoftStatus> {
    return bridge.native ? invoke("microsoft_status") : emptyMicrosoftStatus;
  },
  async connect(
    clientId: string,
    tenantId: string,
    calendarAccess = false,
  ): Promise<void> {
    if (!bridge.native)
      throw new Error("Microsoft sign-in is available in the desktop app.");
    return invoke("connect_microsoft", { clientId, tenantId, calendarAccess });
  },
  async cancel(): Promise<void> {
    if (bridge.native) return invoke("cancel_microsoft_connection");
  },
  async disconnect(): Promise<void> {
    if (bridge.native) return invoke("disconnect_microsoft");
  },
};
