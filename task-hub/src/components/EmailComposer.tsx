import { useState, type FormEvent } from "react";
import {
  ArrowRight,
  ArrowLeft,
  Mail,
  FileCheck2,
  MonitorPlay,
} from "lucide-react";
import { defaults, validateParameters } from "../domain/config";
import type { Parameters, Task } from "../domain/types";
import { AttachmentPicker } from "./AttachmentPicker";
import { validateAttachments, type AttachmentRef } from "../domain/attachments";

export function EmailComposer({
  task,
  native,
  busy,
  senderEmail,
  onRun,
}: {
  task: Task;
  native: boolean;
  busy: boolean;
  senderEmail: string | null;
  onRun: (
    parameters: Parameters,
    attachments: AttachmentRef[],
  ) => Promise<void>;
}) {
  const [values, setValues] = useState<Parameters>(() => ({
    ...defaults(task),
    recipient: "",
    subject: "",
    message: "",
    delivery: "Draft",
  }));
  const [review, setReview] = useState(false);
  const [attachments, setAttachments] = useState<AttachmentRef[]>([]);
  const [attachmentBusy, setAttachmentBusy] = useState(false);
  const [error, setError] = useState("");
  const update = (key: string, value: string) => {
    setValues((old) => ({ ...old, [key]: value }));
    setError("");
  };
  function prepare(event: FormEvent) {
    event.preventDefault();
    setError("");
    try {
      validateParameters(task, values);
      validateAttachments(attachments);
      if (attachmentBusy)
        throw new Error("Wait for the files to finish adding.");
      setReview(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }
  async function start() {
    setError("");
    try {
      validateParameters(task, values);
      if (native && !senderEmail)
        throw new Error(
          "Connect your Gmail account in Gmail connection before starting.",
        );
      validateAttachments(attachments);
      await onRun({ ...values }, attachments);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }
  return (
    <>
      <div className="page-heading compact">
        <span className="eyebrow">ONE TASK, DONE WITH CARE</span>
        <h1>{review ? "Check your email" : "Email automation"}</h1>
        <p>
          {review
            ? "Review the recipient and message before submitting to Gmail."
            : "Create a Gmail draft or send a reviewed email in the background."}
        </p>
      </div>
      <div className="email-layout">
        <section
          className="email-panel"
          aria-label={review ? "Email review" : "Email details"}
        >
          <div className="email-panel-heading">
            <span className="email-step">{review ? "02" : "01"}</span>
            <div>
              <h2>{review ? "Ready for review" : "Compose your message"}</h2>
              <p>Gmail API · background delivery</p>
            </div>
            <Mail size={23} />
          </div>
          {review ? (
            <>
              <dl className="email-review">
                <dt>Recipient</dt>
                <dd>{values.recipient}</dd>
                <dt>Subject</dt>
                <dd>{values.subject}</dd>
                <dt>Message</dt>
                <dd className="email-message">{values.message}</dd>
                <dt>Delivery</dt>
                <dd>
                  {values.delivery === "Draft"
                    ? "Prepare draft"
                    : "Send after review"}
                </dd>
              </dl>
              <AttachmentPicker value={attachments} />
              {native && (
                <p className="notice">
                  From:{" "}
                  {senderEmail ?? "Connect Gmail first using Gmail connection"}
                </p>
              )}
              <p className="email-explanation">
                {!native
                  ? "Preview only: your mailbox will not be opened and no email will be sent."
                  : values.delivery === "Draft"
                    ? "Task Hub will create an unsent draft in Gmail without opening a browser."
                    : "Clicking Send email submits this reviewed message through Gmail’s API. There is no second approval prompt."}
              </p>
              <div className="email-actions">
                <button
                  className="secondary"
                  disabled={busy}
                  onClick={() => {
                    setReview(false);
                    setError("");
                  }}
                >
                  <ArrowLeft size={16} /> Edit email
                </button>
                <button
                  className="primary"
                  disabled={busy}
                  onClick={() => void start()}
                >
                  <MonitorPlay size={16} />
                  {busy
                    ? "Automation running…"
                    : !native
                      ? "Preview email"
                      : values.delivery === "Draft"
                        ? "Prepare draft"
                        : "Send email"}
                </button>
              </div>
            </>
          ) : (
            <form onSubmit={prepare}>
              <fieldset
                disabled={busy || attachmentBusy}
                className="email-fields"
              >
                <div className="email-field">
                  <label htmlFor="email-recipient">Recipient</label>
                  <input
                    id="email-recipient"
                    type="email"
                    autoComplete="off"
                    required
                    maxLength={254}
                    placeholder="teammate@example.com"
                    value={values.recipient as string}
                    onChange={(e) => update("recipient", e.target.value)}
                  />
                </div>
                <div className="email-field">
                  <label htmlFor="email-subject">Subject</label>
                  <input
                    id="email-subject"
                    type="text"
                    required
                    maxLength={160}
                    placeholder="Weekly project update"
                    value={values.subject as string}
                    onChange={(e) => update("subject", e.target.value)}
                  />
                </div>
                <div className="email-field">
                  <label htmlFor="email-message">Message</label>
                  <textarea
                    id="email-message"
                    required
                    rows={9}
                    maxLength={12000}
                    placeholder="Write the message you want Task Hub to enter in Gmail…"
                    value={values.message as string}
                    onChange={(e) => update("message", e.target.value)}
                  />
                </div>
                <AttachmentPicker
                  value={attachments}
                  onChange={setAttachments}
                  onBusyChange={setAttachmentBusy}
                  disabled={busy}
                />
                <div
                  className="email-delivery"
                  role="group"
                  aria-label="Delivery mode"
                >
                  {["Draft", "Send after review"].map((mode) => (
                    <label
                      key={mode}
                      className={`email-mode ${values.delivery === mode ? "selected" : ""}`}
                    >
                      <input
                        type="radio"
                        name="delivery"
                        checked={values.delivery === mode}
                        onChange={() => update("delivery", mode)}
                      />
                      <span>
                        <strong>
                          {mode === "Draft" ? "Prepare draft" : mode}
                        </strong>
                        <small>
                          {mode === "Draft"
                            ? "Save unsent in Gmail"
                            : "Review the message, then click Send email"}
                        </small>
                      </span>
                    </label>
                  ))}
                </div>
                <div className="email-actions">
                  <span className="muted">One recipient · Plain text</span>
                  <button className="primary" type="submit">
                    Review email <ArrowRight size={16} />
                  </button>
                </div>
              </fieldset>
            </form>
          )}
          {error && (
            <p role="alert" className="error">
              {error}
            </p>
          )}
        </section>
        <aside className="email-guide">
          <span className="eyebrow">HOW IT WORKS</span>
          <h2>Your email, sent in the background.</h2>
          <ol>
            <li>
              <strong>Write the details</strong>
              <p>You supply the recipient and exact message.</p>
            </li>
            <li>
              <strong>Review your email</strong>
              <p>Check everything before automation starts.</p>
            </li>
            <li>
              <strong>Use the connected account</strong>
              <p>Gmail API handles the email without desktop input.</p>
            </li>
            <li>
              <strong>Save a draft or send</strong>
              <p>
                Drafts remain unsent. Send email submits the reviewed message.
              </p>
            </li>
          </ol>
          <div className="email-scope">
            <FileCheck2 size={20} />
            <div>
              <strong>Email MVP</strong>
              <p>Gmail API. No browser window needed for delivery.</p>
            </div>
          </div>
        </aside>
      </div>
    </>
  );
}
