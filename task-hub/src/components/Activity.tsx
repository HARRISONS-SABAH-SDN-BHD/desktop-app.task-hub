import {
  Clock3,
  ExternalLink,
  CalendarClock,
  Trash2,
  CheckCircle2,
  AlertCircle,
} from "lucide-react";
import { Help } from "./Help";
import { REPORT_ISSUE_PREFIX } from "../domain/reportRules";
import type { Run, Schedule } from "../domain/types";
export function time(value: string) {
  return new Date(value).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}
export function RunDetail({
  run,
  native,
  onOpen,
}: {
  run: Run;
  native: boolean;
  onOpen: () => void;
}) {
  return (
    <div className="run-detail">
      <div className="section-heading">
        <div>
          <span className="eyebrow">
            {native ? "EXECUTION RESULT" : "BROWSER PREVIEW"}
          </span>
          <h2>{run.title}</h2>
        </div>
        <span className={`status ${run.status}`}>
          {run.status === "succeeded"
            ? native
              ? "Completed"
              : "Previewed"
            : run.status}
        </span>
      </div>
      <p className="muted">{time(run.started_at)}</p>
      {run.error?.startsWith(REPORT_ISSUE_PREFIX) ? (
        <div className="routine-change" role="alert">
          <strong>Email not sent — report needs attention</strong>
          <p>
            Complete and save the report, then return to Email routines and
            select Report ready — retry.
          </p>
          <Help label="Report details">
            {run.error.slice(REPORT_ISSUE_PREFIX.length)}
          </Help>
        </div>
      ) : run.error ? (
        <p role="alert" className="error">
          {run.error}
        </p>
      ) : (
        <p>
          {native
            ? "Email operation completed. See the details below for the delivery result."
            : "Preview completed. No email was sent."}
        </p>
      )}
      <Help label="Execution details">
        <pre className="console" aria-label="Execution log">
          {run.logs.join("\n")}
          {run.error ? `\nERROR: ${run.error}` : ""}
        </pre>
      </Help>
      {run.artifact && (
        <div className="artifact">
          <div>
            <span className="eyebrow">CREATED FILE</span>
            <p>{run.artifact}</p>
          </div>
          <button className="primary" onClick={onOpen}>
            <ExternalLink size={15} /> Open file
          </button>
        </div>
      )}
    </div>
  );
}
export function History({
  runs,
  native,
  onSelect,
}: {
  runs: Run[];
  native: boolean;
  onSelect: (run: Run) => void;
}) {
  return (
    <>
      <div className="page-heading compact">
        <span className="eyebrow">A LITTLE LESS WORK, EVERY TIME</span>
        <h1>Email history</h1>
        <p>
          {native
            ? "A local record of your recent email automation runs."
            : "Preview runs are recorded locally in this browser."}
        </p>
      </div>
      {!runs.length ? (
        <div className="empty">
          <Clock3 size={30} />
          <h3>A fresh start</h3>
          <p>Your email automation results will appear here.</p>
        </div>
      ) : (
        <div className="history-list">
          {runs.map((run) => (
            <button
              key={run.id}
              className="history-row"
              onClick={() => onSelect(run)}
            >
              <span className={`history-symbol ${run.status}`}>
                {run.status === "succeeded" ? (
                  <CheckCircle2 size={19} />
                ) : (
                  <AlertCircle size={19} />
                )}
              </span>
              <div>
                <strong>{run.title}</strong>
                <span>{time(run.started_at)}</span>
              </div>
              <span className={`status ${run.status}`}>
                {run.status === "succeeded" && !native
                  ? "Previewed"
                  : run.status}
              </span>
              <ExternalLink size={15} />
            </button>
          ))}
        </div>
      )}
    </>
  );
}
export function Schedules({
  schedules,
  native,
  onDelete,
}: {
  schedules: Schedule[];
  native: boolean;
  onDelete: (id: string) => void;
}) {
  return (
    <>
      <div className="page-heading compact">
        <span className="eyebrow">GIVE YOUR ROUTINE A RHYTHM</span>
        <h1>Schedules</h1>
        <p>
          Choose “Schedule” on a workflow's review screen to set its next run.
        </p>
      </div>
      <div className="notice">
        <CalendarClock size={19} />
        <span>
          {native
            ? "Schedules run while Task Hub is open. Missed runs execute once when you reopen the app. Recurrences use fixed UTC intervals."
            : "Scheduling is available in the Tauri desktop app. This browser is an interactive preview."}
        </span>
      </div>
      {!schedules.length ? (
        <div className="empty">
          <CalendarClock size={30} />
          <h3>Room for a better routine</h3>
          <p>No schedules yet. Start with a workflow.</p>
        </div>
      ) : (
        <div className="history-list">
          {schedules.map((s) => (
            <div className="history-row" key={s.id}>
              <span className="history-symbol">
                <CalendarClock size={19} />
              </span>
              <div>
                <strong>{s.task.title}</strong>
                <span>
                  {s.enabled ? `Next: ${time(s.next_run)}` : "Completed"} ·{" "}
                  {s.repeat === "daily"
                    ? "Every 24 hours"
                    : s.repeat === "weekly"
                      ? "Every 7 days"
                      : "One time"}
                </span>
              </div>
              <span className={`status ${s.enabled ? "running" : "succeeded"}`}>
                {s.enabled ? "Scheduled" : "Completed"}
              </span>
              <button
                className="icon-button danger"
                aria-label={`Delete schedule for ${s.task.title}`}
                onClick={() => onDelete(s.id)}
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
