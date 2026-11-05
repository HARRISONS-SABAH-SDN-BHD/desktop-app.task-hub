import { useState } from "react";
import { MonitorPlay, OctagonX } from "lucide-react";
import type { ForegroundProgress } from "../domain/types";
export function ForegroundMonitor({
  progress,
  onStop,
  onApprove,
}: {
  progress?: ForegroundProgress | null;
  onStop: (id: string) => Promise<void>;
  onApprove: (id: string) => Promise<void>;
}) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  if (!progress?.active) return null;
  const waiting = progress.phase === "awaiting_send";
  async function act(action: (id: string) => Promise<void>) {
    if (!progress) return;
    setPending(true);
    setError("");
    try {
      await action(progress.run_id);
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }
  return (
    <section
      className="foreground-monitor"
      aria-label="Foreground automation status"
      aria-live="polite"
    >
      <div className="request-heading">
        <MonitorPlay size={21} />
        <div>
          <h2>{progress.label}</h2>
          <p>{progress.message}</p>
        </div>
        <span className="status">
          {waiting ? "Review required" : "Visible automation"}
        </span>
      </div>
      {waiting && progress.review && (
        <div className="send-review">
          <p>
            Check the signed-in account in the browser. Approving authorizes one
            click on Send.
          </p>
          <dl>
            <dt>Mailbox</dt>
            <dd>{progress.review.provider}</dd>
            <dt>To</dt>
            <dd>{progress.review.recipient}</dd>
            <dt>Subject</dt>
            <dd>{progress.review.subject}</dd>
            <dt>Message</dt>
            <dd>{progress.review.body}</dd>
          </dl>
        </div>
      )}
      <pre>{progress.logs.slice(-4).join("\n")}</pre>
      <div className="request-footer">
        <span>
          {waiting
            ? "Approval expires after 3 minutes. Stopping leaves the draft open."
            : "Keep the target app visible. Escape or moving the pointer while typing stops the run."}
        </span>
        <div className="button-row">
          <button
            className="secondary"
            disabled={pending}
            onClick={() => void act(onStop)}
          >
            <OctagonX size={15} /> Stop automation
          </button>
          {waiting && (
            <button
              className="primary"
              disabled={pending}
              onClick={() => void act(onApprove)}
            >
              Approve sending this email
            </button>
          )}
        </div>
      </div>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
