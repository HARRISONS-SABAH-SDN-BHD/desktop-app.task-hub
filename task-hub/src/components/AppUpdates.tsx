import { Help } from "./Help";
import { useEffect, useState } from "react";
import { appUpdates, type UpdateStatus } from "../services/appUpdates";
export function AppUpdates({ busy }: { busy: boolean }) {
  const [status, setStatus] = useState<UpdateStatus>();
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  async function refresh() {
    setStatus(await appUpdates.status());
  }
  useEffect(() => {
    let active = true;
    const poll = () =>
      void appUpdates
        .status()
        .then((value) => {
          if (active) setStatus(value);
        })
        .catch(() => {
          if (active)
            setError(
              "Update status is unavailable. Email routines are unaffected.",
            );
        });
    poll();
    const timer = window.setInterval(poll, 2000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, []);
  async function act(action: () => Promise<void>) {
    setError("");
    setPending(true);
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
      await refresh().catch(() => {});
    }
  }
  const working =
    pending ||
    ["checking", "downloading", "installing"].includes(status?.phase ?? "");
  return (
    <div className="help-content">
      <div className="page-heading compact">
        <span className="eyebrow">TASK HUB · SOFTWARE UPDATES</span>
        <h1>App updates</h1>
        <p>
          Get signed releases without replacing your saved routines or
          attachments.
        </p>
      </div>
      <section
        className="email-panel routine-form"
        aria-label="Task Hub updates"
      >
        <h2>Version {status?.current_version ?? "…"}</h2>
        {!status?.enabled && (
          <p className="notice">
            Updates are available in the installed release. Development and
            browser previews do not install updates.
          </p>
        )}
        <Help label="Automatic update checks">
          The installed app checks automatically after startup and every six
          hours. New releases are downloaded and verified, then wait for your
          click to install and restart. Updates come from published GitHub
          installers; local source changes appear only after running the source
          app or building and publishing a new installer.
        </Help>
        <Help label="Installing safely">
          Finish editing and save your work first. Installation is blocked while
          email or Google operations are running. Task Hub backs up local
          routines and history before installing; your attachments and Google
          connection stay on this computer.
        </Help>
        <p role="status">
          {status?.phase === "ready"
            ? `Version ${status.available_version} is ready to install.`
            : status?.phase === "checking"
              ? "Checking for updates…"
              : status?.phase === "downloading"
                ? "Downloading and verifying the update…"
                : status?.phase === "installing"
                  ? "Starting the installer…"
                  : status?.last_checked
                    ? `Last checked: ${new Date(status.last_checked).toLocaleString()}`
                    : "No update check yet."}
        </p>
        {status?.notes && <p className="email-message">{status.notes}</p>}
        <div className="button-row routine-actions">
          <button
            className="secondary"
            disabled={!status?.enabled || working || status.phase === "ready"}
            onClick={() => void act(appUpdates.check)}
          >
            Check for updates
          </button>
          <button
            className="primary"
            disabled={
              !status?.enabled || status.phase !== "ready" || busy || working
            }
            onClick={() => {
              if (
                window.confirm(
                  "Install the verified update and restart Task Hub? Save your changes first. Scheduled emails pause during installation.",
                )
              )
                void act(appUpdates.install);
            }}
          >
            Install and restart
          </button>
        </div>
        {(error || status?.error) && (
          <p className="error" role="alert">
            {error || status?.error}
          </p>
        )}
        <Help label="When update checks fail">
          If an update check fails, your current app keeps working. Schedule
          timing and delivery rules remain the same.
        </Help>
      </section>
    </div>
  );
}
