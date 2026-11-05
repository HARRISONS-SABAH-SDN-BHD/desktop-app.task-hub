import type { AttachmentRef } from "./attachments";
export type StepKind =
  "select" | "multi_select" | "text" | "textarea" | "email" | "datetime";
export interface Step {
  id: string;
  prompt: string;
  type: StepKind;
  description?: string;
  options?: string[];
  allow_other?: boolean;
  required?: boolean;
  placeholder?: string;
  default?: string | string[];
}
export type Parameters = Record<string, string | string[]>;
export type ExportFormat =
  "eml" | "ics" | "docx" | "xlsx" | "pptx" | "html" | "csv" | "txt";
export type Action =
  | { type: "export"; format: ExportFormat; fields: Record<string, string> }
  | { type: "smtp"; fields: Record<string, string> }
  | {
      type: "foreground";
      recipe: "web_email" | "word_report" | "excel_report" | "powerpoint_deck";
      fields: Record<string, string>;
    }
  | { type: "script"; script_id: "system-info" };
export interface Task {
  id: string;
  title: string;
  description: string;
  category: string;
  icon:
    "mail" | "calendar" | "document" | "sheet" | "slides" | "terminal" | "bell";
  enabled?: boolean;
  steps: Step[];
  action: Action;
}
export interface TaskConfig {
  schema_version: 1;
  tasks: Task[];
}
export interface Run {
  id: string;
  task_id: string;
  title: string;
  status: "running" | "succeeded" | "failed" | "interrupted";
  started_at: string;
  finished_at?: string;
  parameters: Parameters;
  logs: string[];
  artifact?: string;
  error?: string;
}
export interface Schedule {
  id: string;
  task: Task;
  parameters: Parameters;
  next_run: string;
  repeat: "once" | "daily" | "weekly";
  enabled: boolean;
}
export interface Snapshot {
  calendar_links?: import("./emailRoutines").CalendarLink[];
  runs: Run[];
  schedules: Schedule[];
  output_dir: string;
  foreground?: ForegroundProgress | null;
}
export interface ForegroundProgress {
  run_id: string;
  label: string;
  phase: string;
  message: string;
  logs: string[];
  active: boolean;
  review?: {
    provider: string;
    recipient: string;
    subject: string;
    body: string;
  } | null;
}
export interface Bridge {
  native: boolean;
  loadConfig(): Promise<TaskConfig>;
  saveConfig(config: TaskConfig): Promise<void>;
  snapshot(): Promise<Snapshot>;
  run(
    taskId: string,
    parameters: Parameters,
    options?: {
      allowDirectSend?: boolean;
      senderAccount?: string;
      attachments?: AttachmentRef[];
    },
  ): Promise<Run>;
  schedule(
    taskId: string,
    parameters: Parameters,
    nextRun: string,
    repeat: Schedule["repeat"],
  ): Promise<void>;
  deleteSchedule(id: string): Promise<void>;
  openArtifact(runId: string): Promise<void>;
  stopForeground(runId: string): Promise<void>;
  approveForegroundSend(runId: string): Promise<void>;
}
