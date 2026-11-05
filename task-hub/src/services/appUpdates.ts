import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
export interface UpdateStatus {
  enabled: boolean;
  current_version: string;
  phase: "idle" | "checking" | "downloading" | "ready" | "installing" | "error";
  available_version: string | null;
  notes: string | null;
  last_checked: string | null;
  error: string | null;
}
export const appUpdates = {
  status: async (): Promise<UpdateStatus> =>
    bridge.native
      ? invoke("update_status")
      : {
          enabled: false,
          current_version: "0.3.0",
          phase: "idle",
          available_version: null,
          notes: null,
          last_checked: null,
          error: null,
        },
  check: async (): Promise<void> => {
    if (bridge.native) await invoke("check_updates");
  },
  install: async (): Promise<void> => {
    if (bridge.native) await invoke("install_update");
  },
};
