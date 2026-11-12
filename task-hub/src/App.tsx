import { Recipients } from "./components/Recipients";
import { recipientBook } from "./services/recipients";
import type { Recipient } from "./domain/recipients";
import { Connect } from "./components/Connect";
import { ExcelFlow } from "./components/ExcelFlow";
import {
  otherConnections,
  emptyMailServerStatus,
} from "./services/otherConnections";
import { excelFlowWorkspace, idleWorker } from "./services/excelflowWorkspace";
import type { AttachmentRef } from "./domain/attachments";
import { useEffect, useState } from "react";
import {
  Mail,
  History as HistoryIcon,
  CircleHelp,
  ShieldCheck,
  ChevronRight,
  ArrowLeft,
  CalendarClock,
  Download,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import { bridge } from "./services/bridge";
import type { Run, Snapshot } from "./domain/types";
import { History, RunDetail } from "./components/Activity";
import { ForegroundMonitor } from "./components/ForegroundMonitor";
import { AppUpdates } from "./components/AppUpdates";
import { EmailRoutines } from "./components/EmailRoutines";
import { emailRoutines } from "./services/emailRoutines";
import { gmailConnection, type GmailStatus } from "./services/gmailConnection";
import {
  isRoutineDue,
  type EmailRoutine,
  type RoutinePreview,
} from "./domain/emailRoutines";

const pages = [
  { id: "routines", title: "Email routines", icon: CalendarClock },
  { id: "recipients", title: "Recipients", icon: Mail },
  { id: "history", title: "Email history", icon: HistoryIcon },
  { id: "excelflow", title: "ExcelFlow", icon: Mail },
  { id: "connection", title: "Connect", icon: CircleHelp },
  { id: "updates", title: "App updates", icon: Download },
] as const;
type Page = (typeof pages)[number]["id"];

export default function App() {
  const [page, setPage] = useState<Page>("routines");
  const [snapshot, setSnapshot] = useState<Snapshot>({
    runs: [],
    schedules: [],
    output_dir: "",
  });
  const [result, setResult] = useState<Run>();
  const [manualBusy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [recipients, setRecipients] = useState<Recipient[]>([]);
  async function refreshRecipients() {
    setRecipients(await recipientBook.list());
  }
  useEffect(() => {
    void refreshRecipients().catch((e) => setError(String(e)));
  }, []);
  const [routines, setRoutines] = useState<EmailRoutine[]>([]);
  const [mailStatus, setMailStatus] = useState(emptyMailServerStatus);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [worker, setWorker] = useState(idleWorker);
  const [connectionChoice, setConnectionChoice] = useState("gmail");
  const [handoff, setHandoff] = useState<{
    files: AttachmentRef[];
    routineId: string | null;
  } | null>(null);
  const [gmailStatus, setGmailStatus] = useState<GmailStatus>({
    connected: false,
    email: null,
    client_id: "",
    busy: false,
  });
  async function refreshConnection() {
    const [google, mail] = await Promise.all([
      gmailConnection.status(),
      otherConnections.mailStatus(),
    ]);
    setGmailStatus(google);
    setMailStatus(mail);
  }
  const busy =
    manualBusy ||
    worker.active ||
    snapshot.foreground?.active === true ||
    snapshot.runs.some((run) => run.status === "running");
  async function refreshRoutines() {
    setRoutines(await emailRoutines.list());
  }
  useEffect(() => {
    let active = true;
    const refresh = () =>
      bridge
        .snapshot()
        .then((value) => {
          if (active) setSnapshot(value);
        })
        .catch((e) => {
          if (active) setError(String(e));
        });
    void refresh();
    const refreshSaved = () =>
      emailRoutines
        .list()
        .then((value) => {
          if (active) setRoutines(value);
        })
        .catch((e) => {
          if (active) setError(String(e));
        });
    void refreshSaved();
    const refreshAccount = () =>
      gmailConnection
        .status()
        .then((status) => {
          if (active) setGmailStatus(status);
        })
        .catch((e) => {
          if (active) setError(String(e));
        });
    const refreshOthers = () => {
      void otherConnections
        .mailStatus()
        .then((value) => {
          if (active) setMailStatus(value);
        })
        .catch((e) => {
          if (active) setError(String(e));
        });
      void excelFlowWorkspace
        .status()
        .then((value) => {
          if (active) setWorker(value);
        })
        .catch((e) => {
          if (active) setError(String(e));
        });
    };
    refreshOthers();
    void refreshAccount();
    const timer = window.setInterval(() => void refresh(), 750);
    const routineTimer = window.setInterval(() => void refreshSaved(), 5000);
    const accountTimer = window.setInterval(() => {
      void refreshAccount();
      refreshOthers();
    }, 2000);
    return () => {
      active = false;
      clearInterval(timer);
      clearInterval(routineTimer);
      clearInterval(accountTimer);
    };
  }, []);
  function navigate(next: Page) {
    if (busy) return;
    setPage(next);
    setSidebarCollapsed(next === "excelflow");
    setResult(undefined);
    setError("");
  }
  const runs = snapshot.runs.filter((r) => r.task_id === "foreground_email");
  const dueCount = routines.filter(
    (routine) =>
      ["gmail", "mailserver"].includes(routine.settings.provider ?? "gmail") &&
      isRoutineDue(routine),
  ).length;
  async function runRoutine(plan: RoutinePreview) {
    setBusy(true);
    try {
      const outcome = await emailRoutines.run(plan);
      setResult(outcome);
      setSnapshot(await bridge.snapshot());
      await refreshRoutines();
    } finally {
      setBusy(false);
    }
  }
  async function retryReports(id: string, due: string) {
    setBusy(true);
    try {
      const outcome = await emailRoutines.retryReports(id, due);
      setResult(outcome);
      setSnapshot(await bridge.snapshot());
      await refreshRoutines();
    } finally {
      setBusy(false);
    }
  }
  return (
    <div
      className={`app-shell${page === "excelflow" ? " excelflow-shell" : ""}${page === "excelflow" && sidebarCollapsed ? " sidebar-collapsed" : ""}`}
    >
      <aside className="sidebar" id="workspace-sidebar">
        <a
          className="brand"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            navigate("routines");
          }}
        >
          <span className="brand-mark">
            <Mail size={21} />
          </span>
          <span className="brand-name">
            task<span className="brand-light">hub</span>
            <small>EMAIL, ONE TASK AT A TIME</small>
          </span>
        </a>
        <div className="workspace">
          <div className="workspace-avatar">
            <Mail size={18} />
          </div>
          <div>
            <strong>Email workspace</strong>
            <span>
              <span className="tiny-dot" />
              {bridge.native ? "Local desktop" : "Browser preview"}
            </span>
          </div>
        </div>
        <div className="nav-label">WORKSPACE</div>
        <nav>
          {pages.map((item) => (
            <button
              key={item.id}
              aria-label={item.title}
              disabled={busy}
              className={page === item.id ? "active" : ""}
              onClick={() => navigate(item.id)}
            >
              <item.icon size={18} />
              <span>{item.title}</span>
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="local-card">
            <ShieldCheck size={19} />
            <strong>You decide when to send.</strong>
            <p>
              Save email routines as drafts or enable automatic sending on your
              chosen schedule.
            </p>
          </div>
          <div className="engine-status">
            <span className="tiny-dot" />
            {bridge.native ? "Desktop engine connected" : "Preview mode"}
            <span>WORKSPACE</span>
          </div>
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div>
            {page === "excelflow" && (
              <button
                type="button"
                className="sidebar-toggle secondary"
                aria-label={
                  sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"
                }
                aria-expanded={!sidebarCollapsed}
                aria-controls="workspace-sidebar"
                onClick={() => setSidebarCollapsed((value) => !value)}
              >
                {sidebarCollapsed ? (
                  <PanelLeftOpen size={20} />
                ) : (
                  <PanelLeftClose size={20} />
                )}
              </button>
            )}
            Email workspace <ChevronRight size={13} />
            <strong>{pages.find((p) => p.id === page)?.title}</strong>
          </div>
          <span className="runtime-badge">
            <span className="tiny-dot" />
            {bridge.native ? "LOCAL ENGINE" : "INTERACTIVE PREVIEW"}
          </span>
        </header>
        <main>
          {!bridge.native && (
            <div className="preview-strip">
              Browser preview records a preview only. Connect your email account
              through the desktop app for background delivery.
            </div>
          )}
          <div className="content">
            {error && (
              <div role="alert" className="global-error">
                {error}
              </div>
            )}
            <ForegroundMonitor
              progress={snapshot.foreground}
              onStop={bridge.stopForeground}
              onApprove={bridge.approveForegroundSend}
            />
            {bridge.native &&
              !gmailStatus.connected &&
              !mailStatus.connected &&
              page !== "connection" && (
                <div className="routine-due-banner" role="status">
                  <span>
                    Connect an email account to enable background drafts and
                    scheduled sending.
                  </span>
                  <button
                    className="secondary"
                    disabled={busy}
                    onClick={() => navigate("connection")}
                  >
                    Connect Gmail
                  </button>
                </div>
              )}
            {dueCount > 0 && page !== "routines" && !busy && (
              <div className="routine-due-banner" role="status">
                <CalendarClock size={20} />
                <span>
                  {dueCount} saved email{" "}
                  {dueCount === 1 ? "routine is" : "routines are"} due for
                  drafting.
                </span>
                <button
                  className="secondary"
                  onClick={() => navigate("routines")}
                >
                  View due routines
                </button>
              </div>
            )}
            {result ? (
              <>
                <div className="email-result-actions">
                  <button
                    className="back-link"
                    disabled={busy}
                    onClick={() => navigate("routines")}
                  >
                    <ArrowLeft size={16} /> Back to routines
                  </button>
                  <button
                    className="secondary"
                    disabled={busy}
                    onClick={() => navigate("history")}
                  >
                    Email history
                  </button>
                </div>
                <RunDetail
                  run={snapshot.runs.find((r) => r.id === result.id) ?? result}
                  native={bridge.native}
                  onOpen={() =>
                    void bridge
                      .openArtifact(result.id)
                      .catch((e) => setError(String(e)))
                  }
                />
              </>
            ) : page === "routines" ? (
              <EmailRoutines
                routines={routines}
                recipients={recipients}
                runs={snapshot.runs}
                onRetry={retryReports}
                calendarLinks={snapshot.calendar_links ?? []}
                busy={busy}
                handoff={handoff}
                onHandoffUsed={() => setHandoff(null)}
                mailserverEmail={
                  mailStatus.can_send
                    ? (mailStatus.config?.email ?? null)
                    : null
                }
                senderEmail={gmailStatus.email}
                native={bridge.native}
                refresh={refreshRoutines}
                onRun={runRoutine}
              />
            ) : page === "recipients" ? (
              <Recipients
                recipients={recipients}
                refresh={refreshRecipients}
                gmailEmail={gmailStatus.email}
                mailserverEmail={mailStatus.config?.email ?? null}
                native={bridge.native}
                busy={busy}
              />
            ) : page === "history" ? (
              <History
                runs={runs}
                native={bridge.native}
                onSelect={setResult}
              />
            ) : page === "excelflow" ? (
              <ExcelFlow
                native={bridge.native}
                worker={worker}
                routines={routines}
                connected={{
                  gmail: gmailStatus.connected && !!gmailStatus.read_access,
                  mailserver: mailStatus.connected,
                }}
                onConnect={(provider) => {
                  setConnectionChoice(provider);
                  navigate("connection");
                }}
                onUse={(files, routineId) => {
                  setHandoff({ files, routineId });
                  navigate("routines");
                }}
              />
            ) : page === "updates" ? (
              <AppUpdates busy={busy} />
            ) : (
              <Connect
                initialProvider={connectionChoice}
                google={gmailStatus}
                mailserver={mailStatus}
                native={bridge.native}
                busy={busy}
                refresh={refreshConnection}
              />
            )}
          </div>
        </main>
        <footer className="app-footer">
          <span>
            {bridge.native
              ? "Closing the window keeps schedules running. Quit from the system tray."
              : "One task. A clearer workflow."}
          </span>
          <span>GMAIL / IMAP / POP3 · BACKGROUND EMAIL · WINDOWS</span>
        </footer>
      </div>
    </div>
  );
}
