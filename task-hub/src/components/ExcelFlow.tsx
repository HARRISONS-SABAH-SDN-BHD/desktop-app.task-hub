import { useState, useEffect } from "react";
import {
  Home,
  Users,
  Settings,
  FolderOpen,
  FileSpreadsheet,
  Play,
  Search,
  X,
  ExternalLink,
  ClipboardList,
} from "lucide-react";
import { ExcelFlowAgencies } from "./ExcelFlowAgencies";
import { ExcelFlowDialog } from "./ExcelFlowDialog";
import { ExcelFlowMailSource } from "./ExcelFlowMailSource";
import { ExcelFlowPicker } from "./ExcelFlowPicker";
import { reportFiles } from "../services/reportFiles";
import {
  excelFlowWorkspace,
  type WorkbookInput,
  type WorkerStatus,
  type ReportDetails,
} from "../services/excelflowWorkspace";
import type { ExcelFlowRun } from "../domain/excelflow";
import type { AttachmentRef } from "../domain/attachments";
import type { EmailRoutine } from "../domain/emailRoutines";
type View = "menu" | "report" | "details";
type Panel = "source" | "settings" | "agencies" | "results" | null;
const months = Array.from({ length: 12 }, (_, i) =>
  new Date(2026, i).toLocaleString("en", { month: "long" }),
);
export function ExcelFlow({
  native,
  connected,
  routines,
  worker,
  onConnect,
  onUse,
}: {
  native: boolean;
  connected: Record<string, boolean>;
  routines: EmailRoutine[];
  worker: WorkerStatus;
  onConnect: (provider: string) => void;
  onUse: (files: AttachmentRef[], routineId: string | null) => void;
}) {
  const [view, setView] = useState<View>("menu");
  const [panel, setPanel] = useState<Panel>(null);
  const [disc, setDisc] = useState<WorkbookInput | null>(null);
  const [ftg, setFtg] = useState<WorkbookInput | null>(null);
  const [destination, setDestination] = useState<{
    id: string;
    path: string;
  } | null>(null);
  const [scope, setScope] = useState("Both");
  const [mode, setMode] = useState("Both");
  const [autoPeriod, setAutoPeriod] = useState(true);
  const [month, setMonth] = useState(new Date().getMonth() + 1);
  const [year, setYear] = useState(new Date().getFullYear());
  const [money, setMoney] = useState(2);
  const [quantity, setQuantity] = useState(2);
  const [factor, setFactor] = useState("0.88");
  const [repair, setRepair] = useState(true);
  const [agencies, setAgencies] = useState<[string, string][] | null>(null);
  const [run, setRun] = useState<ExcelFlowRun | null>(null);
  const [recent, setRecent] = useState<ExcelFlowRun[]>([]);
  const [target, setTarget] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [details, setDetails] = useState<ReportDetails | null>(null);
  const [tab, setTab] = useState("Date preview");
  const [log, setLog] = useState<string[]>([]);
  const locked = pending || worker.active;
  const targetRoutine = routines.find((r) => r.id === target);
  useEffect(() => {
    void excelFlowWorkspace
      .recent()
      .then(setRecent)
      .catch((e) => setError(String(e)));
  }, []);
  useEffect(() => {
    if (worker.message)
      setLog((rows) =>
        rows.at(-1) === worker.message
          ? rows
          : [...rows.slice(-199), worker.message],
      );
  }, [worker.message]);
  async function act(action: () => Promise<void>) {
    setPending(true);
    setError("");
    setNotice("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
      setLog((rows) => [...rows, String(e)]);
    } finally {
      setPending(false);
    }
  }
  async function generate(validate: boolean) {
    setDetails(null);
    setLog([validate ? "Validating workbooks…" : "Generating reports…"]);
    const result = await excelFlowWorkspace.generate({
      disc: disc?.id ?? null,
      ftg: ftg?.id ?? null,
      scope,
      mode,
      auto_period: autoPeriod,
      month,
      year,
      money_places: money,
      quantity_places: quantity,
      factor,
      repair_dates: repair,
      validate_only: validate,
      destination_id: destination?.id,
    });
    setDetails(result.details ?? null);
    if (result.run) {
      setRun(result.run);
      setRecent(await excelFlowWorkspace.recent());
      setNotice(
        `Reports generated for ${result.run.period}. Choose Open results to use them in an email routine.`,
      );
    } else if (result.validated) {
      setNotice(
        `Validation passed for ${result.period}. ${result.warnings ?? 0} warning(s). No reports or emails were sent.`,
      );
      setView("details");
      setTab("Date preview");
    }
  }
  async function pick(kind: "DISC" | "FTG") {
    const input = await excelFlowWorkspace.pick();
    if (input) (kind === "DISC" ? setDisc : setFtg)(input);
  }
  function chooseAgencies() {
    void act(async () => {
      setAgencies(
        Object.entries((await excelFlowWorkspace.settings()).agencies),
      );
      setPanel("agencies");
    });
  }
  const selectedInputs = (
    [
      ["DISC", disc],
      ["FTG", ftg],
    ] as const
  ).filter(([kind, input]) => input && (scope === "Both" || scope === kind));
  const periods = selectedInputs.map(([kind, input]) => {
    const match =
      /^ALL_(DISC|FTG)_(\d{2})(\d{2})(?:\s*\(\d+\))?\.(xlsx|xlsm)$/i.exec(
        input!.name,
      );
    return match &&
      match[1].toUpperCase() === kind &&
      Number(match[3]) >= 1 &&
      Number(match[3]) <= 12
      ? { month: Number(match[3]), year: 2000 + Number(match[2]) }
      : null;
  });
  const detected =
    periods.length &&
    periods[0] &&
    periods.every(
      (p) => p?.month === periods[0]?.month && p?.year === periods[0]?.year,
    )
      ? periods[0]
      : null;
  const status = worker.active
    ? "Processing reports"
    : error
      ? "Check the issue below"
      : notice
        ? "Completed"
        : "Ready when you are";
  const percent = worker.percent ?? 0;
  return (
    <div className="excel-app">
      {!native && (
        <p className="notice excel-preview">
          Browser preview only. Reading mail and generating Excel workbooks
          require the desktop app.
        </p>
      )}
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {notice && (
        <p role="status" className="notice">
          {notice}
        </p>
      )}
      {view === "menu" ? (
        <div className="excel-menu">
          <h1>Harrison ExcelFlow</h1>
          <p>Choose a module to get started.</p>
          <section className="excel-card excel-module">
            <FileSpreadsheet size={52} aria-hidden="true" />
            <div>
              <h2>DISC &amp; FTG Reports</h2>
              <p>
                Create monthly reports from a DISC workbook, an FTG workbook, or
                both.
                <br />
                Includes date validation, IG name mappings and your saved report
                format.
              </p>
              <button className="primary" onClick={() => setView("report")}>
                <Play size={18} /> DISC &amp; FTG Reports
              </button>
            </div>
          </section>
          <div className="excel-upcoming">
            <h3>More to come</h3>
            <p>New modules and features will be added in future updates.</p>
          </div>
          <p className="excel-menu-footer">
            Harrison ExcelFlow · Excel automation &amp; reporting
          </p>
        </div>
      ) : view === "report" ? (
        <>
          <header className="excel-heading">
            <div>
              <h1>DISC &amp; FTG Reports</h1>
              <p>
                Generate your DISC &amp; FTG monthly reports in a few simple
                steps.
              </p>
            </div>
            <div className="excel-tools">
              <button
                className="secondary"
                disabled={locked}
                onClick={() => setView("menu")}
              >
                <Home size={18} />
                Main menu
              </button>
              <button
                className="secondary"
                disabled={!native || locked}
                onClick={chooseAgencies}
              >
                <Users size={18} />
                Manage IG names
              </button>
              <button
                className="secondary"
                disabled={locked}
                onClick={() => setPanel("settings")}
              >
                <Settings size={18} />
                Settings
              </button>
            </div>
          </header>
          <div className="excel-upper">
            <section
              className="excel-card excel-workbooks"
              aria-label="Workbooks"
            >
              <Section number="01" title="Workbooks" />
              {(
                [
                  ["DISC", disc],
                  ["FTG", ftg],
                ] as const
              ).map(([kind, input]) => (
                <div key={kind} className="excel-input-group">
                  <label htmlFor={`excel-${kind}`}>{kind} workbook</label>
                  <div className="excel-input-row">
                    <div className="excel-path">
                      <FileSpreadsheet size={22} />
                      <input
                        id={`excel-${kind}`}
                        readOnly
                        value={input?.name ?? ""}
                        placeholder="Choose an .xlsx file"
                      />
                    </div>
                    <button
                      className="secondary"
                      disabled={!native || locked}
                      aria-label={`Browse ${kind} workbook`}
                      onClick={() => void act(() => pick(kind))}
                    >
                      <FolderOpen size={18} />
                      Browse…
                    </button>
                  </div>
                </div>
              ))}
              <div className="excel-workbook-hint">
                <span>
                  {autoPeriod
                    ? "The month and year will be detected from your filenames."
                    : "The selected month and year will be used for these reports."}
                </span>
                <button
                  className="back-link"
                  disabled={locked}
                  onClick={() => setPanel("source")}
                >
                  Read from email
                </button>
              </div>
            </section>
            <section
              className="excel-card excel-report-details"
              aria-label="Report details"
            >
              <Section number="02" title="Report details" />
              <div className="excel-period">
                <label>
                  Month
                  <select
                    aria-label="Report month"
                    value={autoPeriod ? (detected?.month ?? "") : month}
                    disabled={autoPeriod || locked}
                    onChange={(e) => setMonth(Number(e.target.value))}
                  >
                    <option value="">Choose workbook first</option>
                    {months.map((name, i) => (
                      <option key={name} value={i + 1}>
                        {name}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Year
                  <input
                    aria-label="Report year"
                    type="number"
                    min={1900}
                    max={2200}
                    value={autoPeriod ? (detected?.year ?? "") : year}
                    disabled={autoPeriod || locked}
                    onChange={(e) => setYear(Number(e.target.value))}
                  />
                </label>
              </div>
              <p className="excel-caption">
                {autoPeriod
                  ? detected
                    ? "Detected automatically from filenames"
                    : selectedInputs.length
                      ? "Check filenames or choose the period in Settings"
                      : "Detected automatically from filenames"
                  : "Selected manually in report settings"}
              </p>
              <Choices
                label="Workbooks to process"
                options={["Both", "DISC", "FTG"]}
                value={scope}
                onChange={setScope}
                disabled={locked}
              />
              <Choices
                label="Report version"
                options={["Both", "Normal", "Special"]}
                value={mode}
                onChange={setMode}
                disabled={locked}
              />
            </section>
          </div>
          <section
            className="excel-card excel-output"
            aria-label="Output location"
          >
            <Section number="03" title="Output location" />
            <div className="excel-output-row">
              <label htmlFor="excel-destination">Save reports to</label>
              <div className="excel-path">
                <FolderOpen size={22} />
                <input
                  id="excel-destination"
                  readOnly
                  value={destination?.path ?? "Task Hub output / ExcelFlow"}
                />
              </div>
              <button
                className="secondary"
                disabled={!native || locked}
                onClick={() =>
                  void act(async () => {
                    const folder = await reportFiles.pick();
                    if (folder) setDestination(folder);
                  })
                }
              >
                <FolderOpen size={18} />
                Choose folder…
              </button>
            </div>
          </section>
          <div className="excel-actions">
            <button
              className="primary"
              disabled={!native || locked || (!disc && !ftg)}
              onClick={() => void act(() => generate(false))}
            >
              <Play size={18} />
              Generate reports
            </button>
            <button
              className="secondary"
              disabled={!native || locked || (!disc && !ftg)}
              onClick={() => void act(() => generate(true))}
            >
              <Search size={18} />
              Validate &amp; preview
            </button>
            <button
              className="secondary"
              disabled={!worker.active || worker.operation !== "generate"}
              onClick={() => void act(() => excelFlowWorkspace.cancel())}
            >
              <X size={18} />
              Cancel
            </button>
            <button className="secondary" onClick={() => setView("details")}>
              <ClipboardList size={18} />
              View run details
            </button>
            <button
              className="secondary excel-open-results"
              onClick={() => setPanel("results")}
            >
              <ExternalLink size={18} />
              Open results
            </button>
          </div>
          <div className="excel-card excel-status" role="status">
            <strong>{status}</strong>
            <span>
              {worker.active
                ? worker.message
                : "Choose a DISC workbook, an FTG workbook, or both to get started."}
            </span>
            <progress max={100} value={percent} />
            <strong>{percent.toFixed(0)}%</strong>
          </div>
        </>
      ) : (
        <>
          <header className="excel-heading">
            <div>
              <h1>Run details</h1>
              <p>Review dates, follow processing, and check any issues.</p>
            </div>
            <button className="secondary" onClick={() => setView("report")}>
              Back to report
            </button>
          </header>
          <section className="excel-card excel-run-details">
            <div role="tablist" aria-label="Run details">
              {["Date preview", "Run log", "Issues"].map((name) => (
                <button
                  key={name}
                  role="tab"
                  aria-selected={tab === name}
                  id={`tab-${name.replaceAll(" ", "-")}`}
                  aria-controls="excel-details-panel"
                  className={tab === name ? "primary" : "secondary"}
                  onClick={() => setTab(name)}
                >
                  {name}
                </button>
              ))}
            </div>
            <div
              role="tabpanel"
              id="excel-details-panel"
              aria-labelledby={`tab-${tab.replaceAll(" ", "-")}`}
            >
              {tab === "Run log" ? (
                <pre>{log.join("\n") || "No run has started yet."}</pre>
              ) : (
                <div className="excel-table-scroll">
                  <table>
                    <thead>
                      <tr>
                        {(tab === "Date preview"
                          ? [
                              "Input",
                              "Excel row",
                              "Original value",
                              "Normalized date",
                              "Result",
                            ]
                          : [
                              "Severity",
                              "Input",
                              "Row",
                              "Column",
                              "Raw value",
                              "Issue",
                            ]
                        ).map((column) => (
                          <th key={column}>{column}</th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {(tab === "Date preview"
                        ? details?.preview
                        : details?.issues
                      )?.map((row, i) => (
                        <tr key={i}>
                          {row.map((cell, j) => (
                            <td key={j}>{String(cell)}</td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {!(tab === "Date preview"
                    ? details?.preview.length
                    : details?.issues.length) && (
                    <div className="excel-empty">
                      <FileSpreadsheet size={48} />
                      <h3>
                        {tab === "Date preview"
                          ? "Your preview will appear here"
                          : "No issues to display"}
                      </h3>
                      <p>
                        Add one or both workbooks, then validate to check dates.
                      </p>
                    </div>
                  )}
                </div>
              )}
            </div>
          </section>
          <div className="excel-card excel-status">
            <strong>{status}</strong>
            <span>{worker.message}</span>
            <strong>{percent.toFixed(0)}%</strong>
          </div>
        </>
      )}
      {panel === "source" && (
        <ExcelFlowDialog
          title="Workbook email source"
          onClose={() => setPanel(null)}
          locked={locked}
        >
          <ExcelFlowMailSource
            native={native}
            connected={connected}
            locked={locked}
            onConnect={onConnect}
            onDisc={setDisc}
            onFtg={setFtg}
          />
          <p>
            DISC: {disc?.name ?? "Not selected"} · FTG:{" "}
            {ftg?.name ?? "Not selected"}
          </p>
        </ExcelFlowDialog>
      )}
      {panel === "settings" && (
        <ExcelFlowDialog
          title="Report settings"
          className="excel-settings-dialog"
          closeButton={false}
          onClose={() => setPanel(null)}
          locked={locked}
        >
          <p>Defaults are set to match your existing reports.</p>
          <fieldset
            disabled={locked}
            className="email-fields excel-settings-fields"
          >
            <label>
              Amount (RM) decimals
              <input
                type="number"
                min={0}
                max={6}
                value={money}
                onChange={(e) => setMoney(Number(e.target.value))}
              />
            </label>
            <p>Displayed monetary precision; also used for P160 rounding.</p>
            <label>
              Quantity decimals
              <input
                type="number"
                min={0}
                max={8}
                value={quantity}
                onChange={(e) => setQuantity(Number(e.target.value))}
              />
            </label>
            <p>Displayed precision; underlying quantities are retained.</p>
            <label>
              P160 multiplier
              <input
                type="number"
                min={0}
                max={10}
                step="0.01"
                value={factor}
                onChange={(e) => setFactor(e.target.value)}
              />
            </label>
            <hr />
            <label className="routine-check">
              <input
                type="checkbox"
                checked={autoPeriod}
                onChange={(e) => setAutoPeriod(e.target.checked)}
              />
              Detect month and year from filenames (YYMM)
            </label>
            <label className="routine-check">
              <input
                type="checkbox"
                checked={repair}
                onChange={(e) => setRepair(e.target.checked)}
              />
              Recover swapped dates using the reporting period
            </label>
            <p>
              For example, October 2026 + 10/13/2026 becomes 13/10/2026.
              <br />
              Turn off filename detection to choose the month and year manually.
            </p>
            <div className="excel-dialog-footer">
              <button
                className="primary"
                disabled={locked}
                onClick={() => setPanel(null)}
              >
                Done
              </button>
            </div>
          </fieldset>
        </ExcelFlowDialog>
      )}
      {panel === "agencies" && agencies && (
        <ExcelFlowAgencies
          initial={agencies}
          locked={locked}
          onClose={() => setPanel(null)}
          onSave={(mapping) =>
            void act(async () => {
              await excelFlowWorkspace.saveAgencies(mapping);
              setPanel(null);
              setNotice("IG names saved for future report generation.");
            })
          }
        />
      )}
      {panel === "results" && (
        <ExcelFlowDialog
          title="Report results"
          onClose={() => setPanel(null)}
          locked={locked}
        >
          {!!recent.length && (
            <label>
              Recent report run
              <select
                aria-label="Recent report run"
                value={run?.folder.id ?? ""}
                onChange={(e) =>
                  setRun(
                    recent.find((r) => r.folder.id === e.target.value) ?? null,
                  )
                }
              >
                <option value="">Choose a completed run</option>
                {recent.map((r) => (
                  <option key={r.folder.id} value={r.folder.id}>
                    {r.period} · {r.folder.path.split(/[\\/]/).pop()}
                  </option>
                ))}
              </select>
            </label>
          )}
          {run && (
            <button
              className="secondary"
              disabled={!native || locked}
              onClick={() => void act(() => reportFiles.open(run.folder))}
            >
              <FolderOpen size={18} />
              Open output folder
            </button>
          )}
          <RoutineTarget
            routines={routines}
            value={target}
            onChange={setTarget}
          />
          <ExcelFlowPicker
            initialRun={run ?? undefined}
            value={targetRoutine?.settings.attachments ?? []}
            onChange={(files) => onUse(files, target || null)}
            disabled={locked}
          />
        </ExcelFlowDialog>
      )}
    </div>
  );
}
function Section({ number, title }: { number: string; title: string }) {
  return (
    <h2 className="excel-section">
      <span>{number}</span>
      {title}
    </h2>
  );
}
function Choices({
  label,
  options,
  value,
  onChange,
  disabled,
}: {
  label: string;
  options: string[];
  value: string;
  onChange: (value: string) => void;
  disabled: boolean;
}) {
  return (
    <fieldset className="excel-choices" disabled={disabled}>
      <legend>{label}</legend>
      <div role="group" aria-label={label}>
        {options.map((option) => (
          <button
            key={option}
            type="button"
            aria-pressed={option === value}
            className={option === value ? "primary" : "secondary"}
            onClick={() => onChange(option)}
          >
            {option}
          </button>
        ))}
      </div>
    </fieldset>
  );
}
function RoutineTarget({
  routines,
  value,
  onChange,
}: {
  routines: EmailRoutine[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="workspace-select">
      Email routine
      <select
        aria-label="Email routine"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      >
        <option value="">Create a new routine</option>
        {routines
          .filter((r) => !r.attempt && r.settings.provider !== "microsoft")
          .map((r) => (
            <option key={r.id} value={r.id}>
              {r.settings.name}
            </option>
          ))}
      </select>
    </label>
  );
}
