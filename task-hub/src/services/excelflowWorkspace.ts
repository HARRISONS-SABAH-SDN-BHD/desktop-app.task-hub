import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
import type { ExcelFlowRun } from "../domain/excelflow";
import type { AttachmentRef } from "../domain/attachments";
export interface WorkbookInput {
  id: string;
  name: string;
  size: number;
}
export interface SourceFolder {
  id: string;
  store_id: string;
  name: string;
}
export interface SourceMessage {
  id: string;
  subject: string;
  sender: string;
  received: string;
  store_id?: string;
}
export interface SourceAttachment {
  id: string;
  name: string;
  size: number;
}
export interface WorkerStatus {
  active: boolean;
  operation: string;
  message: string;
  percent: number | null;
}
export const idleWorker: WorkerStatus = {
  active: false,
  operation: "",
  message: "",
  percent: null,
};
export interface ReportDetails {
  preview: string[][];
  issues: string[][];
  inputs: Record<string, unknown>;
}
export interface GenerateOptions {
  disc: string | null;
  ftg: string | null;
  scope: string;
  mode: string;
  auto_period: boolean;
  month: number;
  year: number;
  money_places: number;
  quantity_places: number;
  factor: string;
  repair_dates: boolean;
  validate_only: boolean;
  destination_id?: string;
}
export const excelFlowWorkspace = {
  async status(): Promise<WorkerStatus> {
    return bridge.native ? invoke("runtime_status") : idleWorker;
  },
  async folders(provider: string): Promise<SourceFolder[]> {
    return bridge.native
      ? invoke("browse_mail_source", {
          provider,
          request: { operation: "folders" },
        })
      : [];
  },
  async messages(
    provider: string,
    folder: SourceFolder,
  ): Promise<SourceMessage[]> {
    return bridge.native
      ? invoke("browse_mail_source", {
          provider,
          request: {
            operation: "messages",
            folder_id: folder.id,
            store_id: folder.store_id,
          },
        })
      : [];
  },
  async attachments(
    provider: string,
    message: SourceMessage,
    folder: SourceFolder,
  ): Promise<SourceAttachment[]> {
    return bridge.native
      ? invoke("browse_mail_source", {
          provider,
          request: {
            operation: "attachments",
            message_id: message.id,
            store_id: message.store_id || folder.store_id,
          },
        })
      : [];
  },
  async download(
    provider: string,
    message: SourceMessage,
    folder: SourceFolder,
    attachment: SourceAttachment,
  ): Promise<WorkbookInput> {
    return invoke("download_mail_workbook", {
      selection: {
        provider,
        message_id: message.id,
        store_id: message.store_id || folder.store_id,
        attachment_id: attachment.id,
        name: attachment.name,
        size: attachment.size,
      },
    });
  },
  async pick(): Promise<WorkbookInput | null> {
    return bridge.native ? invoke("pick_excelflow_input") : null;
  },
  async generate(request: GenerateOptions): Promise<{
    run?: ExcelFlowRun;
    validated?: boolean;
    period?: string;
    warnings?: number;
    details?: ReportDetails;
  }> {
    return invoke("generate_excelflow", { request });
  },
  async recent(): Promise<ExcelFlowRun[]> {
    return bridge.native ? invoke("excelflow_recent_runs") : [];
  },
  async cancel(): Promise<void> {
    return invoke("cancel_report_processing");
  },
  async settings(): Promise<{ agencies: Record<string, string> }> {
    return invoke("excelflow_settings", { agencies: null });
  },
  async saveAgencies(agencies: Record<string, string>): Promise<void> {
    return invoke("excelflow_settings", { agencies });
  },
  async asAttachment(id: string): Promise<AttachmentRef[]> {
    return invoke("workbook_as_attachment", { id });
  },
};
