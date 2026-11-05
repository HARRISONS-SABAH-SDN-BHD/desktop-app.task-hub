import { validateAttachments } from "../domain/attachments";
import { reportFiles } from "./reportFiles";
import { localDate } from "../domain/emailRoutines";
import { validateReportRules } from "../domain/reportRules";
import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
import {
  previewNextDue,
  validateCc,
  previewRoutine,
  renderTemplate,
  type EmailRoutine,
  type RoutinePreview,
  type RoutineSettings,
} from "../domain/emailRoutines";
import type { Run, Snapshot } from "../domain/types";

const storageKey = "taskhub.email-routines.preview.v1";
function read(): EmailRoutine[] {
  return JSON.parse(localStorage.getItem(storageKey) ?? "[]") as EmailRoutine[];
}
function write(routines: EmailRoutine[]) {
  localStorage.setItem(storageKey, JSON.stringify(routines));
}
export const emailRoutines = {
  async retryReports(id: string, due: string): Promise<Run> {
    if (!bridge.native)
      throw new Error(
        "Report completion and retry are available in the desktop app.",
      );
    return invoke("retry_report_email", { id, due });
  },
  async retryCalendar(id: string): Promise<void> {
    if (bridge.native) return invoke("retry_calendar_sync", { id });
  },
  async testNotification(): Promise<void> {
    if (!bridge.native)
      throw new Error(
        "Windows notifications are available in the desktop app.",
      );
    return invoke("test_email_notification");
  },
  async list(): Promise<EmailRoutine[]> {
    if (bridge.native)
      return (
        await invoke<Snapshot & { email_routines: EmailRoutine[] }>(
          "get_snapshot",
        )
      ).email_routines;
    return read();
  },
  async save(id: string | null, settings: RoutineSettings): Promise<void> {
    validateCc(settings.cc ?? "");
    validateReportRules(
      settings.report_rules ?? [],
      settings.attachments?.length ?? 0,
    );
    if (bridge.native) return invoke("save_email_routine", { id, settings });
    const routines = read();
    const old = routines.find((r) => r.id === id);
    if (old?.attempt)
      throw new Error(
        "Handle the existing attempt before editing this routine",
      );
    const scheduleChanged =
      !old ||
      ["start_date", "time", "repeat", "weekdays"].some(
        (key) =>
          JSON.stringify(old.settings[key as keyof RoutineSettings]) !==
          JSON.stringify(settings[key as keyof RoutineSettings]),
      );
    const routine: EmailRoutine = {
      id: id ?? crypto.randomUUID(),
      settings,
      next_due:
        scheduleChanged || (!old?.settings.auto_send && settings.auto_send)
          ? previewNextDue(
              settings,
              settings.auto_send ? new Date() : undefined,
            )
          : old!.next_due,
      attempt: null,
      notified_due: null,
    };
    if (settings.auto_send && !routine.next_due)
      throw new Error(
        "Automatic sending needs a future occurrence. Choose a future date and time.",
      );
    if (routine.next_due) previewRoutine(routine);
    else {
      const day = new Date(`${settings.start_date}T12:00:00`);
      renderTemplate(settings.subject, day);
      renderTemplate(settings.message, day);
    }
    write([...routines.filter((r) => r.id !== id), routine]);
  },
  async preview(id: string): Promise<RoutinePreview> {
    if (bridge.native) return invoke("preview_email_routine", { id });
    const routine = read().find((r) => r.id === id);
    if (!routine) throw new Error("Routine not found");
    return previewRoutine(routine);
  },
  async handle(id: string, due: string): Promise<void> {
    if (bridge.native) return invoke("handle_email_routine", { id, due });
    const routines = read();
    const routine = routines.find((r) => r.id === id);
    if (!routine || routine.next_due !== due)
      throw new Error("Occurrence changed or already handled");
    routine.next_due = previewNextDue(
      routine.settings,
      new Date(Math.max(Date.now(), new Date(due).getTime())),
    );
    routine.attempt = null;
    write(routines);
  },
  async remove(id: string): Promise<void> {
    if (bridge.native) return invoke("delete_email_routine", { id });
    write(read().filter((r) => r.id !== id));
  },
  async run(plan: RoutinePreview): Promise<Run> {
    if (bridge.native)
      return invoke("run_email_routine", {
        id: plan.id,
        due: plan.due,
        parameters: plan.parameters,
      });
    if (plan.parameters.delivery !== "Draft")
      throw new Error(
        "Browser preview cannot run automatic sending. Use the desktop app’s saved schedule.",
      );
    const current = await this.preview(plan.id);
    if (JSON.stringify(current) !== JSON.stringify(plan))
      throw new Error("Routine changed. Open its preview again");
    const answers = { ...plan.parameters };
    delete answers._sender_account;
    delete answers._attachment_ids;
    delete answers._attachment_rules;
    const reports = await reportFiles.previewAttachments(
      read().find((r) => r.id === plan.id)?.settings.report_rules ?? [],
      localDate(new Date(plan.due)),
    );
    validateAttachments([...plan.attachments, ...reports]);
    const run = await bridge.run("foreground_email", answers, {
      attachments: [...plan.attachments, ...reports],
    });
    await this.handle(plan.id, plan.due);
    return run;
  },
};
