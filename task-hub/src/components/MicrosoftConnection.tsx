import { useEffect, useState, type FormEvent } from "react";
import { Help } from "./Help";
import {
  microsoftConnection,
  type MicrosoftStatus,
} from "../services/microsoftConnection";
export function MicrosoftConnection({
  status,
  native,
  busy,
  refresh,
}: {
  status: MicrosoftStatus;
  native: boolean;
  busy: boolean;
  refresh: () => Promise<void>;
}) {
  const [clientId, setClientId] = useState(status.client_id);
  const [tenantId, setTenantId] = useState(status.tenant_id);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    setClientId(status.client_id);
    setTenantId(status.tenant_id);
  }, [status.client_id, status.tenant_id]);
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
  function connect(event: FormEvent) {
    event.preventDefault();
    void act(() =>
      microsoftConnection.connect(
        status.publisher_configured ? "" : clientId.trim(),
        status.publisher_configured ? "" : tenantId.trim(),
      ),
    );
  }
  return (
    <div className="help-content">
      <div className="page-heading compact">
        <span className="eyebrow">WORK EMAIL · MICROSOFT 365</span>
        <h1>Microsoft 365 connection</h1>
        <p>
          Connect your work account to create Outlook drafts and send scheduled
          emails in the background.
        </p>
      </div>
      <section
        className="email-panel routine-form"
        aria-label="Microsoft 365 account connection"
      >
        <h2>
          {status.connected
            ? `Connected: ${status.email}`
            : "Connect your work email"}
        </h2>
        <Help label="Microsoft account access">
          Task Hub requests permission to read your account profile, create
          drafts and send mail as you. Account tokens stay in Windows Credential
          Manager. Company policy may require IT approval. Closing Task Hub to
          the tray keeps schedules running while the computer is awake and
          online.
        </Help>
        {!native && (
          <p className="notice">
            Preview only. Microsoft sign-in and email delivery require the
            desktop app.
          </p>
        )}
        {!status.publisher_configured && (
          <>
            <p className="notice">
              IT must register Task Hub once in Microsoft Entra. These IDs
              identify the application; no client secret or email password is
              needed.
            </p>
            <details className="gmail-setup">
              <summary>Microsoft setup instructions for IT</summary>
              <ol>
                <li>
                  In Microsoft Entra admin center, open App registrations and
                  register Task Hub for accounts in this organization only.
                </li>
                <li>
                  Copy the Application (client) ID and Directory (tenant) ID
                  into the fields below.
                </li>
                <li>
                  Under Authentication, enable Allow public client flows. This
                  version uses Microsoft’s device-code sign-in.
                </li>
                <li>
                  Add Microsoft Graph delegated permissions: User.Read,
                  Mail.ReadWrite and Mail.Send. Approve consent according to
                  company policy. Offline access is requested during sign-in.
                </li>
                <li>
                  Allow Task Hub under your company’s sign-in policies. If
                  device-code sign-in is blocked, ask IT to review the policy
                  rather than disabling security controls.
                </li>
                <li>
                  For distribution, the publisher can include the two IDs in the
                  installer so colleagues only need to connect their accounts.
                </li>
              </ol>
            </details>
          </>
        )}
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        {status.user_code && (
          <div className="notice" role="status">
            <strong>Your Microsoft sign-in code: {status.user_code}</strong>
            <p>
              Enter this code in the Microsoft sign-in page opened in your
              browser. Only enter the code for the connection you just started
              in Task Hub.
            </p>
          </div>
        )}
        <form onSubmit={connect}>
          <fieldset
            className="email-fields"
            disabled={busy || pending || status.busy}
          >
            {!status.publisher_configured && (
              <>
                <label>
                  Application (client) ID
                  <input
                    required
                    value={clientId}
                    onChange={(e) => setClientId(e.target.value)}
                    maxLength={36}
                    placeholder="From IT’s Microsoft Entra registration"
                  />
                </label>
                <label>
                  Directory (tenant) ID
                  <input
                    required
                    value={tenantId}
                    onChange={(e) => setTenantId(e.target.value)}
                    maxLength={36}
                    placeholder="Your company’s directory ID"
                  />
                </label>
              </>
            )}
            <button className="primary" type="submit" disabled={!native}>
              {pending
                ? "Waiting for Microsoft sign-in…"
                : status.connected
                  ? "Reconnect Microsoft 365"
                  : "Connect Microsoft 365"}
            </button>
          </fieldset>
        </form>
        {(pending || status.user_code) && (
          <button
            className="secondary"
            onClick={() => void microsoftConnection.cancel()}
          >
            Cancel Microsoft sign-in
          </button>
        )}
        {status.connected && (
          <button
            className="secondary"
            disabled={busy || pending || status.busy}
            onClick={() => void act(() => microsoftConnection.disconnect())}
          >
            Disconnect Microsoft 365
          </button>
        )}
        <Help label="Attachments and calendar">
          This version supports Microsoft emails up to a 4 MB encoded request,
          usually about 2 MB of attachments. Larger messages stop before
          submission. Microsoft routines do not sync to Google Calendar; Outlook
          calendar sync is not included yet.
        </Help>
      </section>
    </div>
  );
}
