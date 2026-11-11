import { RecipientPicker } from "./RecipientPicker";
import { validateRecipientFile } from "../domain/recipients";
import type { AttachmentRef } from "../domain/attachments";
import { Help } from "./Help";
import { useEffect, useState, type FormEvent } from "react";
import { CalendarClock, Plus, ArrowLeft } from "lucide-react";
import {
  isRoutineDue,
  newRoutine,
  validateCc,
  type EmailRoutine,
  type CalendarLink,
  type RoutinePreview,
  type RoutineSettings,
} from "../domain/emailRoutines";
import { emailRoutines } from "../services/emailRoutines";
import { REPORT_ISSUE_PREFIX } from "../domain/reportRules";
import type { Run } from "../domain/types";
import { AttachmentPicker } from "./AttachmentPicker";
import { validateAttachments } from "../domain/attachments";

const days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const stamp = (value: string) => new Date(value).toLocaleString();
export function EmailRoutines({
  routines,
  runs = [],
  onRetry,
  calendarLinks = [],
  busy,
  native,
  senderEmail,
  mailserverEmail = null,
  refresh,
  onRun,
  handoff,
  onHandoffUsed,
}: {
  routines: EmailRoutine[];
  runs?: Run[];
  onRetry: (id: string, due: string) => Promise<void>;
  calendarLinks?: CalendarLink[];
  busy: boolean;
  native: boolean;
  senderEmail: string | null;
  mailserverEmail?: string | null;
  refresh: () => Promise<void>;
  onRun: (plan: RoutinePreview) => Promise<void>;
  handoff?: { files: AttachmentRef[]; routineId: string | null } | null;
  onHandoffUsed?: () => void;
}) {
  const [editing, setEditing] = useState<{
    id: string | null;
    settings: RoutineSettings;
  } | null>(null);
  const [query, setQuery] = useState("");
  const visibleRoutines = [...routines]
    .reverse()
    .filter(({ settings }) =>
      [
        settings.name,
        settings.recipient,
        settings.cc ?? "",
        settings.subject,
      ].some((value) =>
        value.toLowerCase().includes(query.trim().toLowerCase()),
      ),
    );
  const [plan, setPlan] = useState<RoutinePreview | null>(null);
  const [notice, setNotice] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const locked = busy || pending;
  async function act(action: () => Promise<void>) {
    setPending(true);
    setError("");
    try {
      await action();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }
  async function preview(id: string) {
    await act(async () => {
      setPlan(await emailRoutines.preview(id));
      setEditing(null);
    });
  }
  function edit(routine?: EmailRoutine) {
    setError("");
    setPlan(null);
    setEditing({
      id: routine?.id ?? null,
      settings: routine ? structuredClone(routine.settings) : newRoutine(),
    });
  }
  useEffect(() => {
    if (handoff) {
      const routine = routines.find((r) => r.id === handoff.routineId);
      setPlan(null);
      setEditing({
        id: routine?.id ?? null,
        settings: {
          ...(routine ? structuredClone(routine.settings) : newRoutine()),
          attachments: handoff.files,
        },
      });
      onHandoffUsed?.();
    }
  }, [handoff, routines, onHandoffUsed]);
  const current = plan ? routines.find((r) => r.id === plan.id) : undefined;
  return (
    <>
      <div className="page-heading compact">
        <span className="eyebrow">SAVE ONCE · RUN ON SCHEDULE</span>
        <h1>Email routines</h1>
        <p>
          Save an email once, then prepare a draft or send it automatically on
          your schedule.
        </p>
      </div>
      <p className="routine-essential">
        Keep your computer awake and online for scheduled emails.
      </p>
      <Help label="How scheduled emails work">
        Closing the window keeps Task Hub running in the system tray. Use its
        tray menu to reopen or quit. The computer must stay awake with internet
        access; browsers may be closed and Windows may be locked. Draft routines
        wait for your click; automatic routines send when due. Occurrences over
        five minutes late need rescheduling.
      </Help>
      {native && (
        <div className="routine-actions">
          <button
            className="secondary"
            disabled={locked}
            onClick={() =>
              void act(async () => {
                await emailRoutines.testNotification();
                setNotice(
                  "Notification requested. If no banner appears, check Windows notification settings. The installed app provides Task Hub’s notification identity.",
                );
              })
            }
          >
            Test notification
          </button>
        </div>
      )}
      {notice && <p role="status">{notice}</p>}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {routines.some(
        (r) =>
          !["gmail", "mailserver"].includes(r.settings.provider ?? "gmail"),
      ) && (
        <p className="notice" role="status">
          Routines using an unsupported provider are paused. Their saved details
          are preserved; edit a routine to choose Gmail or IMAP / POP3.
        </p>
      )}
      {editing ? (
        <RoutineForm
          settings={editing.settings}
          mailserverEmail={mailserverEmail}
          senderEmail={senderEmail}
          native={native}
          locked={locked}
          onCancel={() => setEditing(null)}
          onSave={(settings) =>
            void act(async () => {
              await emailRoutines.save(editing.id, settings);
              setQuery("");
              setEditing(null);
            })
          }
        />
      ) : plan ? (
        <section
          className="email-panel routine-review"
          aria-label="Routine email preview"
        >
          <button
            className="back-link"
            disabled={locked}
            onClick={() => setPlan(null)}
          >
            <ArrowLeft size={16} /> Back to routines
          </button>
          <h2>{current?.settings.name}</h2>
          <p>
            Scheduled for {stamp(plan.due)}. Template dates refer to this
            occurrence, even if it is overdue.
          </p>
          {plan.recipient_note && (
            <p className="routine-change" role="status">
              Recipient reminder: {plan.recipient_note}
            </p>
          )}
          <dl className="email-review">
            <dt>From</dt>
            <dd>
              {plan.parameters._sender_account ||
                "Choose a sender by editing and saving the routine"}
            </dd>
            <dt>To</dt>
            <dd>{plan.parameters.recipient}</dd>
            {plan.parameters.cc && (
              <>
                <dt>CC</dt>
                <dd>{plan.parameters.cc}</dd>
              </>
            )}
            <dt>Subject</dt>
            <dd>{plan.parameters.subject}</dd>
            <dt>Message</dt>
            <dd className="email-message">{plan.parameters.message}</dd>
          </dl>
          <AttachmentPicker value={plan.attachments ?? []} />
          {!!current?.settings.report_rules?.length && (
            <p className="notice">
              Legacy report rules are active for this routine. Edit the routine
              to replace them with selected ExcelFlow reports.
            </p>
          )}
          {current?.attempt ? (
            <p className="notice">
              This occurrence was already attempted. Check Email history and
              your provider’s Drafts and Sent folders. It will not run again
              automatically. Use “Mark handled / skip” after checking the
              outcome.
            </p>
          ) : (
            <>
              <p>
                {current?.settings.auto_send
                  ? "Automatic sending is enabled. The saved email will be sent through the selected provider’s API at its scheduled time, without an approval prompt."
                  : "Task Hub creates the draft through the selected provider’s API without opening a browser. Nothing is sent. Edit the routine to change future drafts."}
              </p>
              {!current?.settings.auto_send && (
                <button
                  className="primary"
                  disabled={locked || !current || !isRoutineDue(current)}
                  onClick={() =>
                    void act(async () => {
                      await onRun(plan);
                      setPlan(null);
                    })
                  }
                >
                  {busy
                    ? "Preparing email…"
                    : native
                      ? "Prepare draft"
                      : "Preview routine only"}
                </button>
              )}
              {current && !isRoutineDue(current) && (
                <p>
                  This occurrence is not due yet. You can preview its resolved
                  content here.
                </p>
              )}
            </>
          )}
        </section>
      ) : (
        <>
          <div className="routine-toolbar">
            <button
              className="primary"
              disabled={locked}
              onClick={() => edit()}
            >
              <Plus size={16} /> New email routine
            </button>
            {!!routines.length && (
              <label className="routine-search">
                <span>Find a routine</span>
                <input
                  type="search"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="Search name, recipient or subject"
                />
              </label>
            )}
          </div>
          {!!routines.length && (
            <p className="muted" role="status">
              {visibleRoutines.length} of {routines.length} routines · Newest
              first
            </p>
          )}
          {!!routines.length && !visibleRoutines.length && (
            <p className="empty">
              No matching routines. Try another name, email address or subject.
            </p>
          )}
          {!routines.length && (
            <div className="empty">
              <CalendarClock size={30} />
              <h2>No routines yet</h2>
              <p>
                Save a monthly report email, its recipient, template and
                schedule once.
              </p>
            </div>
          )}
          <div className="routine-list">
            {visibleRoutines.map((routine) => {
              const { settings } = routine;
              const due = isRoutineDue(routine);
              const missedAutomatic =
                due &&
                settings.auto_send &&
                !!routine.next_due &&
                Date.now() - new Date(routine.next_due).getTime() > 300_000;
              const change = settings.recipient_change;
              const attachmentFailure = runs.find(
                (run) =>
                  run.id === routine.attempt &&
                  run.status === "failed" &&
                  run.error?.startsWith(REPORT_ISSUE_PREFIX),
              );
              const calendarLink = calendarLinks.find(
                (link) =>
                  link.routine_id === routine.id &&
                  (link.provider ?? "gmail") ===
                    (settings.provider ?? "gmail") &&
                  link.sender.toLowerCase() ===
                    settings.sender_account?.toLowerCase(),
              );
              return (
                <article className="email-panel routine-card" key={routine.id}>
                  <div className="routine-card-heading">
                    <h2>{settings.name}</h2>
                    <span className={`status ${due ? "routine-due" : ""}`}>
                      {!settings.enabled
                        ? "Paused"
                        : !routine.next_due
                          ? "Completed"
                          : routine.attempt
                            ? "Check outcome"
                            : due
                              ? settings.auto_send
                                ? missedAutomatic
                                  ? "Missed · reschedule"
                                  : "Automatic send due"
                                : "Due for drafting"
                              : "Upcoming"}
                    </span>
                  </div>
                  <p className="muted">
                    {!["gmail", "mailserver"].includes(
                      settings.provider ?? "gmail",
                    )
                      ? "Unsupported provider"
                      : settings.provider === "mailserver"
                        ? "IMAP / POP3"
                        : "Gmail"}
                  </p>
                  <p>
                    Recipient: <strong>{settings.recipient}</strong>
                  </p>
                  <p>
                    <strong>
                      {settings.auto_send
                        ? "Automatic send · no approval required"
                        : "Draft only · prepare when ready"}
                    </strong>
                  </p>
                  <p>
                    {settings.repeat === "weekly"
                      ? `Weekly · ${settings.weekdays.map((day) => days[day]).join(", ")}`
                      : settings.repeat}{" "}
                    · {settings.time} · computer’s local time
                    {settings.repeat === "monthly"
                      ? ` · day ${Number(settings.start_date.slice(-2))} (last day in shorter months)`
                      : ""}
                  </p>
                  <p>
                    {routine.next_due
                      ? `Next occurrence: ${stamp(routine.next_due)}`
                      : "No remaining occurrences."}
                  </p>
                  {change && (
                    <p className="routine-change">
                      Recipient change: {change.recipient} from {change.from}
                      {change.until
                        ? ` through ${change.until}; then return to ${settings.recipient}`
                        : " permanently"}
                      . {change.note}
                    </p>
                  )}
                  <details className="routine-details">
                    <summary>Routine details</summary>
                    {settings.cc && (
                      <p>
                        CC: <strong>{settings.cc}</strong>
                      </p>
                    )}
                    {(settings.calendar?.enabled || calendarLink) && (
                      <p className={calendarLink?.error ? "error" : "muted"}>
                        {!["gmail", "mailserver"].includes(
                          settings.provider ?? "gmail",
                        )
                          ? "Unsupported calendar"
                          : "Google Calendar"}
                        :{" "}
                        {!native
                          ? "Preview only — no event created"
                          : (calendarLink?.error ??
                            (calendarLink?.synced_hash
                              ? "Synced"
                              : "Sync pending."))}
                      </p>
                    )}
                    {native && calendarLink?.error && (
                      <button
                        className="secondary"
                        disabled={locked}
                        onClick={() =>
                          void act(() =>
                            emailRoutines.retryCalendar(routine.id),
                          )
                        }
                      >
                        Retry calendar sync
                      </button>
                    )}
                    {!!settings.attachments?.length && (
                      <p>
                        Attachments:{" "}
                        {settings.attachments
                          .map((file) => file.name)
                          .join(", ")}
                      </p>
                    )}
                    <p>
                      Sender:{" "}
                      <strong>
                        {settings.sender_account ||
                          "Not selected — edit and save this routine after connecting an email account"}
                      </strong>
                    </p>
                    {!!settings.report_rules?.length && (
                      <p className="notice">
                        Legacy report rules are active. Edit this routine to
                        remove them.
                      </p>
                    )}
                  </details>
                  {attachmentFailure && (
                    <div className="routine-change">
                      <p role="alert">
                        <strong>Email not sent — report needs attention</strong>
                      </p>
                      <p>
                        Complete and save the report, then select Report ready —
                        retry.
                      </p>
                      <Help label="Report details">
                        {attachmentFailure.error}
                      </Help>
                    </div>
                  )}
                  {routine.attempt && !attachmentFailure && (
                    <p className="error">
                      An attempt is recorded. Inspect history and provider
                      Drafts / Sent before handling this occurrence.
                    </p>
                  )}
                  {missedAutomatic && !routine.attempt && (
                    <p className="routine-change">
                      The automatic send window was missed. Edit the date or
                      time to reschedule; this occurrence will not send late.
                    </p>
                  )}
                  <div className="button-row routine-actions">
                    {due &&
                      !routine.attempt &&
                      !settings.auto_send &&
                      ["gmail", "mailserver"].includes(
                        settings.provider ?? "gmail",
                      ) && (
                        <button
                          className="primary"
                          disabled={locked}
                          onClick={() =>
                            void act(async () => {
                              await onRun(
                                await emailRoutines.preview(routine.id),
                              );
                            })
                          }
                        >
                          {native ? "Prepare draft" : "Preview routine only"}
                        </button>
                      )}
                    {attachmentFailure && routine.next_due && (
                      <button
                        className="primary"
                        disabled={
                          locked ||
                          !settings.enabled ||
                          !["gmail", "mailserver"].includes(
                            settings.provider ?? "gmail",
                          )
                        }
                        onClick={() =>
                          void act(() => onRetry(routine.id, routine.next_due!))
                        }
                      >
                        Report ready — retry{" "}
                        {settings.auto_send ? "sending" : "draft"}
                      </button>
                    )}
                    {routine.next_due && (
                      <button
                        className="secondary"
                        disabled={
                          locked ||
                          !["gmail", "mailserver"].includes(
                            settings.provider ?? "gmail",
                          )
                        }
                        onClick={() => void preview(routine.id)}
                      >
                        {due ? "Preview due email" : "Preview next email"}
                      </button>
                    )}
                    <button
                      className="secondary"
                      disabled={
                        locked || (!!routine.attempt && !attachmentFailure)
                      }
                      onClick={() => edit(routine)}
                    >
                      Edit routine / recipient
                    </button>
                    <details className="routine-manage">
                      <summary>More actions</summary>
                      {routine.next_due && (
                        <button
                          className="secondary"
                          disabled={locked}
                          onClick={() => {
                            if (
                              window.confirm(
                                "Mark this occurrence handled without sending? Missed dates will be skipped and the next future occurrence will be scheduled.",
                              )
                            )
                              void act(() =>
                                emailRoutines.handle(
                                  routine.id,
                                  routine.next_due!,
                                ),
                              );
                          }}
                        >
                          Mark handled / skip
                        </button>
                      )}
                      <button
                        className="secondary"
                        disabled={locked}
                        onClick={() => {
                          if (
                            window.confirm(
                              `Delete the saved routine “${settings.name}”? Email history will remain.`,
                            )
                          )
                            void act(() => emailRoutines.remove(routine.id));
                        }}
                      >
                        Delete routine
                      </button>
                    </details>
                  </div>
                </article>
              );
            })}
          </div>
        </>
      )}
    </>
  );
}

function RoutineForm({
  settings: initial,
  native,
  senderEmail,
  mailserverEmail = null,
  locked,
  onSave,
  onCancel,
}: {
  settings: RoutineSettings;
  native: boolean;
  senderEmail: string | null;
  mailserverEmail?: string | null;
  locked: boolean;
  onSave: (settings: RoutineSettings) => void;
  onCancel: () => void;
}) {
  const [settings, setSettings] = useState(() => ({
    ...initial,
    provider:
      initial.provider ??
      (senderEmail || !mailserverEmail ? "gmail" : "mailserver"),
    report_rules: initial.report_rules ?? [],
    calendar: initial.calendar ?? { enabled: false, reminder_minutes: 1440 },
    cc: initial.cc ?? "",
    attachments: initial.attachments ?? [],
    sender_account:
      initial.sender_account ??
      (native ? (senderEmail ?? mailserverEmail) : "preview@example.com"),
  }));
  const [error, setError] = useState("");
  const [attachmentBusy, setAttachmentBusy] = useState(false);
  function update<K extends keyof RoutineSettings>(
    key: K,
    value: RoutineSettings[K],
  ) {
    setSettings((s) => ({ ...s, [key]: value }));
  }
  function save(event: FormEvent) {
    event.preventDefault();
    setError("");
    if (!["gmail", "mailserver"].includes(settings.provider ?? "gmail")) {
      setError(
        "This provider is unsupported. Choose a connected Gmail or IMAP / POP3 sender before saving.",
      );
      return;
    }
    if (
      native &&
      (!settings.sender_account ||
        settings.sender_account !==
          (settings.provider === "mailserver" ? mailserverEmail : senderEmail))
    ) {
      setError(
        "Connect the intended email sender, then choose that account before saving this routine.",
      );
      return;
    }
    if (settings.repeat === "weekly" && !settings.weekdays.length) {
      setError("Choose at least one weekday.");
      return;
    }
    try {
      validateCc(settings.cc);
      validateAttachments(settings.attachments);
      validateRecipientFile(settings.recipient_file, settings.recipient);
    } catch (e) {
      setError(String(e));
      return;
    }
    if (attachmentBusy) {
      setError("Wait for the files to finish adding.");
      return;
    }
    onSave({ ...settings, report_rules: [], recipient_change: null });
  }
  return (
    <form className="email-panel routine-form" onSubmit={save}>
      <h2>Set up your email routine</h2>
      <Help label="Setting up a routine">
        Choose who receives the email, what to include, and when it should run.
        Optional settings can be left as they are.
      </Help>
      {(!!initial.report_rules?.length || !!initial.recipient_change) && (
        <p className="notice">
          Saving this routine removes its previous report rules and recipient
          changes. Confirm the selected recipients and add all files you want to
          send below.
        </p>
      )}
      <fieldset disabled={locked || attachmentBusy} className="email-fields">
        <h3 className="form-section-title">1. Email details</h3>
        <label>
          Sender account
          <select
            aria-label="Sender account"
            required
            value={
              settings.sender_account
                ? (settings.provider ?? "gmail") + ":" + settings.sender_account
                : ""
            }
            onChange={(event) => {
              const [provider, ...parts] = event.target.value.split(":");
              setSettings((current) => ({
                ...current,
                provider: provider === "mailserver" ? "mailserver" : "gmail",
                sender_account: parts.join(":") || null,
                calendar:
                  provider === "mailserver"
                    ? {
                        enabled: false,
                        reminder_minutes: current.calendar.reminder_minutes,
                      }
                    : current.calendar,
              }));
            }}
          >
            <option value="">Choose a connected sender</option>
            {settings.sender_account &&
              settings.sender_account !==
                (!["gmail", "mailserver"].includes(settings.provider ?? "gmail")
                  ? null
                  : settings.provider === "mailserver"
                    ? mailserverEmail
                    : senderEmail) && (
                <option
                  value={
                    (settings.provider ?? "gmail") +
                    ":" +
                    settings.sender_account
                  }
                >
                  {settings.sender_account}{" "}
                  {native ? "(not connected)" : "(preview only)"}
                </option>
              )}
            {!native && !senderEmail && (
              <option value="gmail:preview@example.com">
                Gmail — preview@example.com (preview only)
              </option>
            )}
            {senderEmail && (
              <option value={"gmail:" + senderEmail}>
                Gmail — {senderEmail}
              </option>
            )}
            {mailserverEmail && (
              <option value={"mailserver:" + mailserverEmail}>
                IMAP / POP3 — {mailserverEmail}
              </option>
            )}
          </select>
        </label>
        <label>
          Routine name
          <input
            required
            maxLength={100}
            value={settings.name}
            onChange={(e) => update("name", e.target.value)}
            placeholder="Monthly progress report"
          />
        </label>
        <RecipientPicker
          file={settings.recipient_file}
          selected={settings.recipient}
          disabled={locked}
          onBusyChange={setAttachmentBusy}
          onChange={(file, selected) =>
            setSettings((s) => ({
              ...s,
              recipient_file: file,
              recipient: selected,
            }))
          }
        />
        <label>
          CC (optional)
          <input
            type="email"
            multiple
            aria-label="CC (optional)"
            maxLength={8000}
            value={settings.cc}
            onChange={(e) => update("cc", e.target.value)}
            placeholder="colleague@example.com, team@example.com"
          />
          <small>Separate multiple email addresses with commas.</small>
        </label>
        <h3 className="form-section-title">2. Write your message</h3>
        <label>
          Subject template
          <input
            required
            maxLength={160}
            value={settings.subject}
            onChange={(e) => update("subject", e.target.value)}
          />
        </label>
        <label>
          Message template
          <textarea
            required
            rows={8}
            maxLength={12000}
            value={settings.message}
            onChange={(e) => update("message", e.target.value)}
          />
        </label>
        <h3 className="form-section-title">3. Add files (optional)</h3>
        <Help label="Choosing attachments">
          Use a saved copy for files that stay the same. Add files saves a copy
          for this routine; changing the original file does not update that
          copy. Remove it and add the updated file when needed.
        </Help>
        <AttachmentPicker
          title="Attachments"
          value={settings.attachments}
          onChange={(items) => update("attachments", items)}
          onBusyChange={setAttachmentBusy}
          disabled={locked}
        />
        <Help label="Message date fields">
          Available fields:{" "}
          <code>
            {
              "{{month}} · {{year}} · {{date}} · {{previous_month}} · {{previous_year}}"
            }
          </code>
          . Dates come from the scheduled occurrence. Templates reuse your text;
          they do not generate report figures or read files.
        </Help>
        <h3 className="form-section-title">4. Choose the schedule</h3>
        <div className="routine-form-grid">
          <label>
            First date
            <input
              type="date"
              required
              value={settings.start_date}
              onChange={(e) => update("start_date", e.target.value)}
            />
          </label>
          <label>
            Time
            <input
              type="time"
              required
              value={settings.time}
              onChange={(e) => update("time", e.target.value)}
            />
          </label>
          <label>
            Repeat
            <select
              value={settings.repeat}
              onChange={(e) =>
                update("repeat", e.target.value as RoutineSettings["repeat"])
              }
            >
              <option value="once">Once</option>
              <option value="daily">Daily</option>
              <option value="weekly">Weekly on selected days</option>
              <option value="monthly">Monthly</option>
            </select>
          </label>
        </div>
        {settings.repeat === "weekly" && (
          <div className="routine-weekdays" role="group" aria-label="Weekdays">
            {days.map((day, index) => (
              <label key={day}>
                <input
                  type="checkbox"
                  checked={settings.weekdays.includes(index)}
                  onChange={(e) =>
                    update(
                      "weekdays",
                      e.target.checked
                        ? [...settings.weekdays, index].sort()
                        : settings.weekdays.filter((d) => d !== index),
                    )
                  }
                />
                {day}
              </label>
            ))}
          </div>
        )}
        <label className="routine-check">
          <input
            type="checkbox"
            checked={settings.enabled}
            onChange={(e) => update("enabled", e.target.checked)}
          />{" "}
          Routine enabled
        </label>
        <label className="routine-check">
          <input
            type="checkbox"
            checked={settings.auto_send ?? false}
            onChange={(e) => update("auto_send", e.target.checked)}
          />{" "}
          Automatically send when due
        </label>
        {settings.auto_send && (
          <div className="routine-change">
            <strong>Emails will be sent automatically when due.</strong>
            <Help label="Automatic sending">
              Saving authorizes Task Hub to send the saved recipient and message
              on each scheduled occurrence without asking again. It uses this
              saved sender account through the selected provider’s API in the
              background. Failed or uncertain sends are not retried
              automatically. Enabling automatic sending schedules the next
              future occurrence.
            </Help>
          </div>
        )}
        <h3 className="form-section-title">5. Calendar reminder (optional)</h3>
        {settings.provider === "mailserver" && (
          <p>Calendar sync is available for Gmail accounts.</p>
        )}
        <label className="routine-check">
          <input
            type="checkbox"
            disabled={settings.provider !== "gmail"}
            checked={settings.calendar.enabled}
            onChange={(e) =>
              update("calendar", {
                ...settings.calendar,
                enabled: e.target.checked,
              })
            }
          />
          Sync to Google Calendar
        </label>
        {settings.calendar.enabled && (
          <>
            <label>
              Calendar reminder
              <select
                aria-label="Calendar reminder"
                value={settings.calendar.reminder_minutes}
                onChange={(e) =>
                  update("calendar", {
                    ...settings.calendar,
                    reminder_minutes: Number(e.target.value),
                  })
                }
              >
                <option value={0}>At the scheduled time</option>
                <option value={15}>15 minutes before</option>
                <option value={60}>1 hour before</option>
                <option value={1440}>1 day before</option>
                <option value={10080}>1 week before</option>
              </select>
            </label>
            <Help label="Calendar reminders">
              Creates a private reminder in your connected account’s primary
              calendar. For Gmail, enable Google Calendar API and reconnect with
              Calendar access. Changes sync from Task Hub; edit routines here.
              Disabling sync, disabling the routine, or deleting it removes its
              calendar event when Task Hub is running and connected. Email
              contents, recipients and files are not copied to the calendar.
            </Help>
          </>
        )}
      </fieldset>
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      <div className="button-row routine-actions">
        <button
          type="button"
          className="secondary"
          disabled={locked}
          onClick={onCancel}
        >
          Cancel
        </button>
        <button
          className="primary"
          disabled={locked || attachmentBusy}
          type="submit"
        >
          Save routine
        </button>
      </div>
    </form>
  );
}
