import { Help } from "./Help";
import { useEffect, useState, type FormEvent } from "react";
import { gmailConnection, type GmailStatus } from "../services/gmailConnection";

export function GmailConnection({
  status,
  native,
  busy,
  refresh,
}: {
  status: GmailStatus;
  native: boolean;
  busy: boolean;
  refresh: () => Promise<void>;
}) {
  const [clientId, setClientId] = useState(status.client_id);
  const [clientSecret, setClientSecret] = useState("");
  const [calendarAccess, setCalendarAccess] = useState(
    status.calendar_access ?? false,
  );
  useEffect(() => {
    if (status.calendar_access) setCalendarAccess(true);
  }, [status.calendar_access]);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (status.client_id) setClientId(status.client_id);
  }, [status.client_id]);
  async function act(action: () => Promise<void>) {
    setError("");
    setPending(true);
    try {
      await action();
      await refresh();
      setClientSecret("");
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }
  function connect(event: FormEvent) {
    event.preventDefault();
    void act(() =>
      gmailConnection.connect(clientId.trim(), clientSecret, calendarAccess),
    );
  }
  return (
    <div className="help-content">
      <div className="page-heading compact">
        <span className="eyebrow">BACKGROUND EMAIL · GOOGLE SIGN-IN</span>
        <h1>Gmail connection</h1>
        <p>
          Connect once. Drafts and scheduled emails use Gmail’s API without
          opening a browser window.
        </p>
      </div>
      <section
        className="email-panel routine-form"
        aria-label="Gmail account connection"
      >
        <h2>
          {status.connected
            ? `Connected: ${status.email}`
            : "Connect your Gmail account"}
        </h2>
        <Help label="Background sending">
          Closing the window keeps Task Hub running in the system tray. Use the
          tray menu to reopen or quit it. Internet access is required; browsers
          may be closed and Windows may be locked. A powered-off or sleeping
          computer cannot run schedules.
        </Help>
        <Help label="Account security">
          Refresh tokens and the desktop client secret are stored in Windows
          Credential Manager. They are never saved in routines, email history,
          or browser storage.
        </Help>
        {!native && (
          <p className="notice">
            Preview only. No Google account is connected and no emails are sent.
            Configure and connect through the desktop app.
          </p>
        )}
        <details className="gmail-setup">
          <summary>Google Cloud setup instructions</summary>
          <ol>
            <li>
              Create or select a Google Cloud project and enable{" "}
              <strong>Gmail API</strong>. For calendar sync, also enable{" "}
              <strong>Google Calendar API</strong>.
            </li>
            <li>
              In <strong>Google Auth Platform</strong>, configure branding and
              audience. For an External app in Testing, add your Gmail address
              as a test user.
            </li>
            <li>
              Configure data access for <code>openid</code>, <code>email</code>,
              and <code>https://www.googleapis.com/auth/gmail.compose</code>.
              Gmail compose access allows creating drafts and sending email. For
              optional calendar sync, also configure{" "}
              <code>https://www.googleapis.com/auth/calendar.events.owned</code>
              , then reconnect with Calendar access enabled.
            </li>
            <li>
              Create an OAuth client with application type{" "}
              <strong>Desktop app</strong>. Copy its client ID and, if provided,
              its client secret into the fields below. Use desktop credentials,
              not a web application client.
            </li>
            <li>
              Click <strong>Connect Gmail</strong>, finish Google sign-in and
              consent in your browser, then return here. Edit and save existing
              routines to bind them to this connected sender.
            </li>
          </ol>
          <Help label="Sign-in expiry">
            Google may expire refresh tokens after seven days for External
            projects in Testing. Reconnect when needed. Distribution beyond test
            users may require Google verification.
          </Help>
          <div className="button-row routine-actions">
            <button
              className="secondary"
              onClick={() => void act(() => gmailConnection.openSetup("cloud"))}
              disabled={pending}
            >
              Open Google Cloud setup
            </button>
            <button
              className="secondary"
              onClick={() => void act(() => gmailConnection.openSetup("oauth"))}
              disabled={pending}
            >
              Google’s desktop OAuth guide
            </button>
            <button
              className="secondary"
              onClick={() =>
                void act(() => gmailConnection.openSetup("calendar"))
              }
              disabled={pending}
            >
              Enable Google Calendar API
            </button>
          </div>
        </details>
        <p>
          Google Calendar:{" "}
          {status.calendar_access
            ? "Access granted. Enable sync in individual routines."
            : "Not connected. Enable Calendar access below and reconnect to grant it."}
        </p>
        <form onSubmit={connect}>
          <fieldset
            disabled={busy || pending || status.busy}
            className="email-fields"
          >
            <label>
              Desktop OAuth client ID
              <input
                required
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                autoComplete="off"
                placeholder="123456-example.apps.googleusercontent.com"
                maxLength={250}
              />
            </label>
            <label>
              Desktop client secret (if provided)
              <input
                type="password"
                value={clientSecret}
                onChange={(e) => setClientSecret(e.target.value)}
                autoComplete="off"
                maxLength={1024}
              />
            </label>
            <label className="routine-check">
              <input
                type="checkbox"
                checked={calendarAccess}
                onChange={(e) => setCalendarAccess(e.target.checked)}
              />
              Enable Google Calendar access
            </label>
            <button className="primary" type="submit" disabled={!native}>
              {pending
                ? "Waiting for Google sign-in…"
                : status.connected
                  ? "Reconnect Gmail"
                  : "Connect Gmail"}
            </button>
          </fieldset>
        </form>
        {pending && (
          <button
            className="secondary"
            onClick={() => void gmailConnection.cancel()}
          >
            Cancel sign-in
          </button>
        )}
        {status.connected && (
          <button
            className="secondary routine-disconnect"
            disabled={busy || pending || status.busy}
            onClick={() => void act(() => gmailConnection.disconnect())}
          >
            Disconnect locally
          </button>
        )}
        <Help label="Disconnecting your account">
          Disconnect locally removes the stored credentials and stops background
          delivery. You can also remove Task Hub’s access in your Google
          account’s connected-app settings.
        </Help>
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
      </section>
    </div>
  );
}
