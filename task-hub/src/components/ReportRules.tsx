import { Help } from "./Help";
import { useRef, useState } from "react";
import { AttachmentPicker } from "./AttachmentPicker";
import { bridge } from "../services/bridge";
import { reportFiles } from "../services/reportFiles";
import { localDate } from "../domain/emailRoutines";
import type {
  PatternSuggestion,
  ReportRule,
  RuleCheck,
} from "../domain/reportRules";
export function ReportRules({
  value,
  onChange,
  disabled = false,
  onBusyChange,
}: {
  value: ReportRule[];
  onChange?: (rules: ReportRule[]) => void;
  disabled?: boolean;
  onBusyChange?: (busy: boolean) => void;
}) {
  return (
    <section className="report-rules" aria-label="Current report attachments">
      <h3>Current report files</h3>
      <Help label="Finding current reports">
        Find a different report for each occurrence in a folder you select.
        Filename suggestions use names only; no report contents are studied or
        sent to an AI service. Subfolders are not searched. Remove an old report
        from fixed attachments when replacing it with a current-report rule.
      </Help>
      {value.map((rule, index) => (
        <ReportRuleEditor
          key={rule.id}
          rule={rule}
          number={index + 1}
          disabled={disabled}
          onBusyChange={onBusyChange}
          onChange={
            onChange
              ? (updated) =>
                  onChange(value.map((r) => (r.id === rule.id ? updated : r)))
              : undefined
          }
          onRemove={
            onChange
              ? () => onChange(value.filter((r) => r.id !== rule.id))
              : undefined
          }
        />
      ))}
      {onChange && (
        <button
          className="secondary"
          type="button"
          disabled={disabled || value.length >= 10}
          onClick={() =>
            onChange([
              ...value,
              {
                id: crypto.randomUUID(),
                folder: { id: "", path: "" },
                pattern: "",
                previous_month: false,
                template: null,
                require_changed: true,
              },
            ])
          }
        >
          Add current report rule
        </button>
      )}
    </section>
  );
}
function ReportRuleEditor({
  rule,
  number,
  onChange,
  onRemove,
  disabled,
  onBusyChange,
}: {
  rule: ReportRule;
  number: number;
  onChange?: (rule: ReportRule) => void;
  onRemove?: () => void;
  disabled: boolean;
  onBusyChange?: (busy: boolean) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [suggestions, setSuggestions] = useState<PatternSuggestion[]>([]);
  const [date, setDate] = useState(localDate());
  const [result, setResult] = useState<RuleCheck>();
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  function update(change: Partial<ReportRule>) {
    onChange?.({ ...rule, ...change });
    setResult(undefined);
    setError("");
  }
  async function act(action: () => Promise<void>) {
    setPending(true);
    onBusyChange?.(true);
    setError("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
      onBusyChange?.(false);
    }
  }
  async function selectFolder(folder: ReportRule["folder"] | null) {
    if (!folder) return;
    update({ folder });
    setSuggestions(await reportFiles.suggest(folder));
  }
  if (!onChange)
    return (
      <div className="routine-change">
        <strong>
          Report {number}: {rule.pattern}
        </strong>
        <p>
          {rule.folder.path.replace(/^\\\\\?\\/, "")} ·{" "}
          {rule.previous_month
            ? "Previous month (last day)"
            : "Scheduled occurrence date"}
        </p>
        {bridge.native && (
          <button
            className="secondary"
            type="button"
            onClick={() => void act(() => reportFiles.open(rule.folder))}
          >
            Open report folder
          </button>
        )}
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <Help label="Missing report behavior">
          {rule.template
            ? `If missing: create a copy of ${rule.template.name} and wait for completion.`
            : "If missing: stop without sending."}{" "}
          Files are resolved again when the email runs.
        </Help>
      </div>
    );
  return (
    <div className="routine-change" aria-label={`Report rule ${number}`}>
      <strong>Report {number}</strong>
      <p className="report-path">
        {rule.folder.path.replace(/^\\\\\?\\/, "") ||
          "Choose the folder containing this report."}
      </p>
      <button
        type="button"
        className="secondary"
        disabled={disabled || pending}
        onClick={() =>
          bridge.native
            ? void act(async () => selectFolder(await reportFiles.pick()))
            : input.current?.click()
        }
      >
        Choose report folder
      </button>
      <input
        ref={input}
        hidden
        type="file"
        multiple
        {...{ webkitdirectory: "" }}
        aria-label={`Report folder files ${number}`}
        onChange={(e) => {
          const files = Array.from(e.target.files ?? []);
          e.target.value = "";
          void act(async () => selectFolder(reportFiles.previewFolder(files)));
        }}
      />
      {rule.folder.id && (
        <button
          type="button"
          className="secondary"
          disabled={disabled || pending}
          onClick={() =>
            void act(async () =>
              setSuggestions(await reportFiles.suggest(rule.folder)),
            )
          }
        >
          Suggest patterns
        </button>
      )}
      {suggestions.length > 0 && (
        <div className="report-suggestions">
          <p>Suggested patterns — choose one, then test it:</p>
          {suggestions.map((s) => (
            <button
              type="button"
              className="secondary"
              key={s.pattern}
              disabled={disabled || pending}
              onClick={() => update({ pattern: s.pattern })}
            >
              <span>{s.pattern}</span>
              <small>
                {s.count} filename{s.count === 1 ? "" : "s"}:{" "}
                {s.samples.join(", ")}
              </small>
            </button>
          ))}
        </div>
      )}
      <label>
        Filename pattern
        <input
          aria-label={`Filename pattern ${number}`}
          required
          maxLength={600}
          value={rule.pattern}
          placeholder="{{MONTH_SHORT}} report {{year}}.xlsx"
          onChange={(e) => update({ pattern: e.target.value })}
        />
      </label>
      <Help label="Filename patterns">
        Use <code>{"{{date}}"}</code> for 5-11-2026 or{" "}
        <code>{"{{iso_date}}"}</code> for 2026-11-05. Examples:{" "}
        <code>{"{{day}}-{{month_number}}-{{year}}.xlsx"}</code> or{" "}
        <code>{"{{MONTH_SHORT}} report {{year}}.pdf"}</code>. Use{" "}
        <code>{"{{dd}}"}</code> / <code>{"{{mm}}"}</code> for leading zeros.{" "}
        <code>*</code> matches extra text, but more than one match stops the
        email.
      </Help>
      <label>
        Report period
        <select
          aria-label={`Report period ${number}`}
          value={rule.previous_month ? "previous" : "scheduled"}
          onChange={(e) =>
            update({ previous_month: e.target.value === "previous" })
          }
        >
          <option value="scheduled">Scheduled occurrence date</option>
          <option value="previous">Previous month (last day)</option>
        </select>
      </label>
      <label className="routine-check">
        <input
          type="checkbox"
          checked={rule.require_changed}
          onChange={(e) => update({ require_changed: e.target.checked })}
        />
        Require updated contents since this routine’s last successful email
      </label>
      <AttachmentPicker
        title="Missing-report template (optional)"
        ariaLabel={`Report template ${number}`}
        maxFiles={1}
        value={rule.template ? [rule.template] : []}
        onChange={(items) => update({ template: items[0] ?? null })}
        disabled={disabled || pending}
        onBusyChange={onBusyChange}
      />
      <Help label="Missing report templates">
        If no report exists, Task Hub can create a copy of this template with
        the expected filename. The extension must match, and the pattern must be
        exact. Complete and save the new report, then explicitly retry the
        email. Template contents are copied; report figures are not invented.
        Existing files are never overwritten or renamed.
      </Help>
      <div className="routine-time-grid">
        <label>
          Test report date
          <input
            aria-label={`Test report date ${number}`}
            type="date"
            value={date}
            onChange={(e) => {
              setDate(e.target.value);
              setResult(undefined);
            }}
          />
        </label>
        <button
          className="secondary"
          type="button"
          disabled={disabled || pending || !rule.folder.id || !rule.pattern}
          onClick={() =>
            void act(async () =>
              setResult(await reportFiles.inspect(rule, date)),
            )
          }
        >
          Test report rule
        </button>
      </div>
      {result && (
        <div role="status">
          <p>
            Expected: <strong>{result.expected}</strong>
          </p>
          <p>{result.status}</p>
          {result.matches.map((name) => (
            <p key={name}>{name}</p>
          ))}
        </div>
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <button
        type="button"
        className="secondary"
        disabled={disabled || pending}
        onClick={onRemove}
      >
        Remove report rule
      </button>
    </div>
  );
}
