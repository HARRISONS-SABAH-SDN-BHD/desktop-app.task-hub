import { useRef, useState } from "react";
import { Help } from "./Help";
import { bridge } from "../services/bridge";
import { reportFiles } from "../services/reportFiles";
import { excelFlow } from "../services/excelflow";
import type { ExcelFlowRun } from "../domain/excelflow";
import { attachmentSize, type AttachmentRef } from "../domain/attachments";
export function ExcelFlowPicker({
  value,
  onChange,
  disabled = false,
  onBusyChange,
}: {
  value: AttachmentRef[];
  onChange: (value: AttachmentRef[]) => void;
  disabled?: boolean;
  onBusyChange?: (value: boolean) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [run, setRun] = useState<ExcelFlowRun | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [filter, setFilter] = useState("");
  const [version, setVersion] = useState("All");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  async function act(action: () => Promise<void>) {
    setPending(true);
    onBusyChange?.(true);
    setError("");
    setNotice("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
      onBusyChange?.(false);
    }
  }
  function show(next: ExcelFlowRun) {
    setRun(next);
    setSelected([]);
    setFilter("");
    setVersion("All");
  }
  const shown =
    run?.reports.filter(
      (r) =>
        (version === "All" || r.version === version) &&
        [r.name, r.branch, r.kind].some((v) =>
          v.toLowerCase().includes(filter.trim().toLowerCase()),
        ),
    ) ?? [];
  return (
    <section className="excelflow-picker" aria-label="ExcelFlow report import">
      <h3>ExcelFlow reports</h3>
      <Help label="Using ExcelFlow reports">
        Generate your DISC or FTG reports in Harrison ExcelFlow, then choose its
        completed run folder here. Select the branch, type and version you want.
        Task Hub saves copies as fixed attachments; new monthly runs do not
        replace these copies. Edit the routine to replace them. No email is sent
        by importing. Choose one Normal or Special version per filename.
      </Help>
      <div className="button-row routine-actions">
        <button
          type="button"
          className="secondary"
          disabled={disabled || pending}
          onClick={() => {
            if (!bridge.native) {
              input.current?.click();
              return;
            }
            void act(async () => {
              const folder = await reportFiles.pick();
              if (folder) show(await excelFlow.inspect(folder.id));
            });
          }}
        >
          Choose ExcelFlow run
        </button>
        {bridge.native && (
          <>
            <button
              type="button"
              className="secondary"
              disabled={disabled || pending}
              onClick={() => void act(() => excelFlow.launch())}
            >
              Open ExcelFlow
            </button>
            <button
              type="button"
              className="secondary"
              disabled={disabled || pending}
              onClick={() =>
                void act(async () => {
                  const folder = await reportFiles.pick();
                  if (folder) {
                    await excelFlow.configure(folder.id);
                    setNotice(
                      "ExcelFlow application location saved. You can now open ExcelFlow from Task Hub.",
                    );
                  }
                })
              }
            >
              Set ExcelFlow app folder
            </button>
          </>
        )}
      </div>
      {!bridge.native && (
        <input
          ref={input}
          type="file"
          multiple
          {...{ webkitdirectory: "" }}
          aria-label="ExcelFlow run folder"
          hidden
          onChange={(e) => {
            const files = [...(e.target.files ?? [])];
            e.target.value = "";
            if (files.length)
              void act(async () => show(await excelFlow.preview(files)));
          }}
        />
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
      {run && (
        <>
          <p>
            <strong>Report month: {run.period}</strong> · {run.reports.length}{" "}
            generated reports
          </p>
          <p className="muted">{run.folder.path}</p>
          <div className="routine-form-grid">
            <label>
              Find ExcelFlow reports
              <input
                type="search"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder="Branch, DISC, FTG or filename"
              />
            </label>
            <label>
              ExcelFlow report version
              <select
                value={version}
                onChange={(e) => setVersion(e.target.value)}
              >
                <option>All</option>
                <option>Normal</option>
                <option>Special</option>
              </select>
            </label>
          </div>
          <p>
            {selected.length} selected ·{" "}
            {attachmentSize(
              run.reports
                .filter((r) => selected.includes(r.key))
                .reduce((n, r) => n + r.size, 0),
            )}
          </p>
          <div
            className="excelflow-report-list"
            role="group"
            aria-label="Generated ExcelFlow reports"
          >
            {shown.map((report) => (
              <label className="excelflow-report" key={report.key}>
                <input
                  type="checkbox"
                  disabled={
                    disabled ||
                    pending ||
                    (!selected.includes(report.key) &&
                      selected.length + value.length >= 10)
                  }
                  checked={selected.includes(report.key)}
                  onChange={(e) =>
                    setSelected((current) =>
                      e.target.checked
                        ? [...current, report.key]
                        : current.filter((k) => k !== report.key),
                    )
                  }
                />
                <span>
                  <strong>{report.name}</strong>
                  <span>
                    {report.branch} · {report.kind} · {report.version} ·{" "}
                    {report.rows.toLocaleString()} rows ·{" "}
                    {attachmentSize(report.size)}
                  </span>
                </span>
              </label>
            ))}
            {!shown.length && <p>No matching reports.</p>}
          </div>
          <button
            type="button"
            className="primary"
            disabled={disabled || pending || !selected.length}
            onClick={() =>
              void act(async () => {
                const items = await excelFlow.import(run, selected, value);
                onChange([...value, ...items]);
                setSelected([]);
                setNotice(
                  `Added ${items.length} ExcelFlow report${items.length === 1 ? "" : "s"} to fixed attachments. Save the routine to keep them.`,
                );
              })
            }
          >
            {pending ? "Importing reports…" : "Add selected reports"}
          </button>
        </>
      )}
    </section>
  );
}
