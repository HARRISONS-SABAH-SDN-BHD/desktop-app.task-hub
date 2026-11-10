import { useState } from "react";
import { GmailConnection } from "./GmailConnection";
import { Help } from "./Help";
import {
  otherConnections,
  type MailConfig,
  type MailServerStatus,
} from "../services/otherConnections";
import type { GmailStatus } from "../services/gmailConnection";
const defaultConfig: MailConfig = {
  email: "",
  protocol: "imap",
  host: "",
  port: 993,
  username: "",
  folder: "INBOX",
  draft_folder: "Drafts",
  smtp_host: "",
  smtp_port: 587,
  smtp_username: "",
  smtp_security: "starttls",
};
export function Connect({
  initialProvider = "gmail",
  google,
  mailserver,
  native,
  busy,
  refresh,
}: {
  initialProvider?: string;
  google: GmailStatus;
  mailserver: MailServerStatus;
  native: boolean;
  busy: boolean;
  refresh: () => Promise<void>;
}) {
  const [provider, setProvider] = useState(
    initialProvider === "mailserver" ? "mailserver" : "gmail",
  );
  return (
    <>
      <div className="page-heading compact">
        <span className="eyebrow">ACCOUNTS & SOURCES</span>
        <h1>Connect</h1>
        <p>Connect Gmail or another email service using IMAP / POP3.</p>
      </div>
      <div
        className="connection-options"
        role="group"
        aria-label="Connection type"
      >
        {[
          ["gmail", "Gmail"],
          ["mailserver", "IMAP / POP3"],
        ].map(([id, name]) => (
          <button
            type="button"
            key={id}
            aria-pressed={provider === id}
            disabled={busy}
            className={provider === id ? "primary" : "secondary"}
            onClick={() => setProvider(id)}
          >
            {name}
          </button>
        ))}
      </div>
      {provider === "gmail" ? (
        <GmailConnection
          embedded
          status={google}
          native={native}
          busy={busy}
          refresh={refresh}
        />
      ) : (
        <MailServerForm
          key={mailserver.config?.email ?? "new"}
          status={mailserver}
          native={native}
          busy={busy}
          refresh={refresh}
        />
      )}
    </>
  );
}
function MailServerForm({
  status,
  native,
  busy,
  refresh,
}: {
  status: MailServerStatus;
  native: boolean;
  busy: boolean;
  refresh: () => Promise<void>;
}) {
  const [config, setConfig] = useState(status.config ?? defaultConfig);
  const [password, setPassword] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  function update<K extends keyof MailConfig>(key: K, value: MailConfig[K]) {
    setConfig((c) => ({ ...c, [key]: value }));
  }
  async function connect(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      await otherConnections.connectMail(config, password);
      setPassword("");
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }
  return (
    <section
      className="email-panel routine-form"
      aria-label="IMAP POP3 connection"
    >
      <h2>IMAP / POP3</h2>
      <p>
        {status.connected
          ? `Connected: ${status.config?.email}`
          : "Connect another email provider using its server settings."}
      </p>
      {status.connected && (
        <p>
          {status.can_send
            ? "Incoming mail and outgoing SMTP connected."
            : "Incoming mail connected. Add SMTP settings to send routines."}
        </p>
      )}
      <Help label="Mail server settings">
        Ask IT for incoming IMAP or POP3 settings. Incoming connections require
        encrypted TLS. Outgoing SMTP is optional for reading attachments, but
        required to send routines. Use a provider-issued app password where
        required. Your provider may block password-based access; ask IT which
        connection methods your account supports. Credentials are stored in
        Windows Credential Manager, never in routine files. Browsing does not
        delete messages or mark them read.
      </Help>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <form onSubmit={(event) => void connect(event)}>
        <fieldset className="email-fields" disabled={busy || pending}>
          <label>
            Email address
            <input
              type="email"
              required
              value={config.email}
              onChange={(e) => update("email", e.target.value)}
              autoComplete="username"
            />
          </label>
          <label>
            Incoming protocol
            <select
              aria-label="Incoming protocol"
              value={config.protocol}
              onChange={(e) =>
                setConfig((c) => ({
                  ...c,
                  protocol: e.target.value as "imap" | "pop3",
                  port: e.target.value === "imap" ? 993 : 995,
                }))
              }
            >
              <option value="imap">IMAP (recommended)</option>
              <option value="pop3">POP3</option>
            </select>
          </label>
          <div className="routine-form-grid">
            <label>
              Incoming server
              <input
                required
                value={config.host}
                onChange={(e) => update("host", e.target.value)}
                placeholder="imap.example.com"
              />
            </label>
            <label>
              Incoming TLS port
              <input
                type="number"
                required
                min={1}
                max={65535}
                value={config.port}
                onChange={(e) => update("port", Number(e.target.value))}
              />
            </label>
          </div>
          <label>
            Incoming username
            <input
              required
              value={config.username}
              onChange={(e) => update("username", e.target.value)}
              autoComplete="off"
            />
          </label>
          <label>
            Password or app password
            <input
              required
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="new-password"
              maxLength={2048}
            />
          </label>
          {config.protocol === "imap" && (
            <div className="routine-form-grid">
              <label>
                Incoming folder
                <input
                  value={config.folder}
                  onChange={(e) => update("folder", e.target.value)}
                />
              </label>
              <label>
                Drafts folder
                <input
                  value={config.draft_folder}
                  onChange={(e) => update("draft_folder", e.target.value)}
                />
              </label>
            </div>
          )}
          <details>
            <summary>Outgoing SMTP (optional)</summary>
            <p>
              Add SMTP settings if this account should send email routines. The
              same password is used; SMTP username may differ.
            </p>
            <label>
              SMTP server
              <input
                value={config.smtp_host}
                onChange={(e) => update("smtp_host", e.target.value)}
                placeholder="smtp.example.com"
              />
            </label>
            <div className="routine-form-grid">
              <label>
                SMTP port
                <input
                  type="number"
                  min={1}
                  max={65535}
                  value={config.smtp_port}
                  onChange={(e) => update("smtp_port", Number(e.target.value))}
                />
              </label>
              <label>
                SMTP security
                <select
                  aria-label="SMTP security"
                  value={config.smtp_security}
                  onChange={(e) =>
                    setConfig((c) => ({
                      ...c,
                      smtp_security: e.target.value as "tls" | "starttls",
                      smtp_port: e.target.value === "tls" ? 465 : 587,
                    }))
                  }
                >
                  <option value="starttls">STARTTLS (required)</option>
                  <option value="tls">TLS</option>
                </select>
              </label>
            </div>
            <label>
              SMTP username
              <input
                value={config.smtp_username}
                onChange={(e) => update("smtp_username", e.target.value)}
                placeholder="Same as incoming if blank"
              />
            </label>
          </details>
          <button type="submit" className="primary" disabled={!native}>
            {pending
              ? "Testing connection…"
              : status.connected
                ? "Reconnect mail server"
                : "Connect mail server"}
          </button>
        </fieldset>
      </form>
      {status.connected && (
        <button
          type="button"
          className="secondary"
          disabled={busy || pending}
          onClick={() =>
            void otherConnections
              .disconnectMail()
              .then(refresh)
              .catch((e) => setError(String(e)))
          }
        >
          Disconnect mail server
        </button>
      )}
      {!native && (
        <p className="notice">
          Connection testing is available in the desktop app. No passwords are
          saved in browser preview.
        </p>
      )}
      <Help label="Drafts and Sent Items">
        IMAP drafts use the configured Drafts folder. POP3 cannot upload drafts;
        Task Hub saves a local .eml draft instead. SMTP submission does not
        automatically create a Sent Items copy. Calendar sync is available
        through Gmail or Microsoft 365.
      </Help>
    </section>
  );
}
