import type { ReportRule } from "./reportRules";
import type { Parameters } from "./types";
import type { AttachmentRef } from "./attachments";

import type { RecipientFile } from "./recipients";

export interface RecipientChange {
  recipient: string;
  from: string;
  until: string | null;
  note: string;
}
export interface CalendarLink {
  provider?: string;
  routine_id: string;
  sender: string;
  event_id: string;
  synced_hash: string | null;
  error: string | null;
}
export interface RoutineSettings {
  provider?: string;
  report_rules?: ReportRule[];
  calendar?: { enabled: boolean; reminder_minutes: number } | null;
  cc?: string;
  name: string;
  recipient: string;
  recipient_file?: RecipientFile;
  subject: string;
  message: string;
  start_date: string;
  time: string;
  repeat: "once" | "daily" | "weekly" | "monthly";
  weekdays: number[];
  enabled: boolean;
  auto_send: boolean;
  sender_account: string | null;
  attachments: AttachmentRef[];
  recipient_change: RecipientChange | null;
}
export interface EmailRoutine {
  id: string;
  settings: RoutineSettings;
  next_due: string | null;
  attempt: string | null;
  notified_due: string | null;
}
export interface RoutinePreview {
  id: string;
  due: string;
  parameters: Parameters;
  recipient_note: string;
  attachments: AttachmentRef[];
}
export function localDate(date = new Date()): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
export function newRoutine(): RoutineSettings {
  return {
    name: "",
    report_rules: [],
    calendar: { enabled: false, reminder_minutes: 1440 },
    recipient: "",
    cc: "",
    subject: "{{previous_month}} {{previous_year}} report",
    message:
      "Hello,\n\nHere is the report for {{previous_month}} {{previous_year}}.\n\nRegards,",
    start_date: localDate(),
    time: "09:00",
    repeat: "monthly",
    weekdays: [0],
    enabled: true,
    auto_send: false,
    sender_account: null,
    attachments: [],
    recipient_change: null,
  };
}
export function renderTemplate(template: string, due: Date): string {
  const previous = new Date(due.getFullYear(), due.getMonth(), 0);
  const month = (date: Date) => date.toLocaleString("en-US", { month: "long" });
  const values: Record<string, string> = {
    month: month(due),
    year: String(due.getFullYear()),
    date: `${String(due.getDate()).padStart(2, "0")} ${month(due)} ${due.getFullYear()}`,
    previous_month: month(previous),
    previous_year: String(previous.getFullYear()),
  };
  const rendered = template.replace(/\{\{(.*?)\}\}/g, (_, field: string) => {
    if (!Object.hasOwn(values, field.trim()))
      throw new Error(`Unknown template field: ${field}`);
    return values[field.trim()];
  });
  if (rendered.includes("{{")) throw new Error("Unclosed template field");
  return rendered;
}
// Browser-preview simulation only. Native Rust owns production scheduling,
// recurrence, occurrence claiming, rendering and durable history.
export function previewNextDue(
  settings: RoutineSettings,
  after?: Date,
): string | null {
  const start = new Date(`${settings.start_date}T00:00:00`);
  const [hour, minute] = settings.time.split(":").map(Number);
  let day = new Date(start);
  if (after && day < after)
    day = new Date(after.getFullYear(), after.getMonth(), after.getDate());
  for (let offset = 0; offset < 3660; offset++) {
    const last = new Date(day.getFullYear(), day.getMonth() + 1, 0).getDate();
    const match =
      settings.repeat === "daily" ||
      (settings.repeat === "once" && localDate(day) === settings.start_date) ||
      (settings.repeat === "weekly" &&
        settings.weekdays.includes((day.getDay() + 6) % 7)) ||
      (settings.repeat === "monthly" &&
        day.getDate() === Math.min(start.getDate(), last));
    const stamp = new Date(
      day.getFullYear(),
      day.getMonth(),
      day.getDate(),
      hour,
      minute,
    );
    if (
      match &&
      stamp.getHours() === hour &&
      stamp.getMinutes() === minute &&
      (!after || stamp > after)
    )
      return stamp.toISOString();
    if (settings.repeat === "once") return null;
    day.setDate(day.getDate() + 1);
  }
  return null;
}
export function previewRoutine(routine: EmailRoutine): RoutinePreview {
  if (!routine.next_due) throw new Error("Routine has no remaining date");
  const due = new Date(routine.next_due);
  const day = localDate(due);
  const settings = routine.settings;
  const change = settings.recipient_change;
  const active =
    change && day >= change.from && (!change.until || day <= change.until);
  return {
    id: routine.id,
    due: routine.next_due,
    parameters: {
      recipient: active ? change.recipient : settings.recipient,
      cc: settings.cc ?? "",
      subject: renderTemplate(settings.subject, due),
      message: renderTemplate(settings.message, due),
      delivery: settings.auto_send ? "Send directly" : "Draft",
      _provider: settings.provider ?? "gmail",
      _sender_account: settings.sender_account ?? "",
      _attachment_rules: JSON.stringify(settings.report_rules ?? []),
      _attachment_ids: (settings.attachments ?? []).map((a) => a.id).join(","),
    },
    recipient_note: active
      ? `Recipient changed from ${settings.recipient} to ${change.recipient}. ${change.until ? `Temporary through ${change.until}; then the original recipient returns.` : "Permanent from the effective date."} ${change.note}`
      : "",
    attachments: settings.attachments ?? [],
  };
}
export function isRoutineDue(routine: EmailRoutine): boolean {
  return (
    routine.settings.enabled &&
    !!routine.next_due &&
    new Date(routine.next_due).getTime() <= Date.now()
  );
}

export function validateCc(value: string): void {
  if (!value.trim()) return;
  const addresses = value.split(",").map((address) => address.trim());
  if (
    value.length > 8000 ||
    /[\r\n\x00-\x1f\x7f]/.test(value) ||
    addresses.length > 100 ||
    addresses.some(
      (address) => !/^[^\s<>,;"@]+@[^\s<>,;"@]+\.[^\s<>,;"@]+$/.test(address),
    )
  )
    throw new Error(
      "Enter valid CC email addresses separated by commas (up to 100), or leave CC blank.",
    );
}
