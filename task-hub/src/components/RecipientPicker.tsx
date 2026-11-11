import { useRef, useState } from "react";
import { importRecipientFile } from "../services/recipients";
import type { RecipientFile } from "../domain/recipients";
export function RecipientPicker({
  file,
  selected,
  onChange,
  disabled,
  onBusyChange,
}: {
  file?: RecipientFile;
  selected: string;
  onChange: (file: RecipientFile | undefined, selected: string) => void;
  disabled: boolean;
  onBusyChange: (busy: boolean) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const recipients =
    file?.recipients ??
    selected
      .split(",")
      .map((v) => v.trim())
      .filter(Boolean);
  const sourceFile = file ?? { name: "Saved recipients", recipients };
  const checked = new Set(
    selected
      .split(",")
      .map((v) => v.trim())
      .filter(Boolean),
  );
  async function load(source: File) {
    setBusy(true);
    onBusyChange(true);
    setError("");
    try {
      const imported = await importRecipientFile(source);
      onChange(imported, imported.recipients.join(", "));
      setQuery("");
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
      onBusyChange(false);
      if (input.current) input.current.value = "";
    }
  }
  return (
    <section className="recipient-picker" aria-label="Recipient">
      <h4>Recipient</h4>
      <p className="muted">
        One email is sent to the selected addresses. Everyone in To can see the
        other recipients.
      </p>
      <input
        ref={input}
        type="file"
        accept=".csv,.txt,.xlsx"
        aria-label="Recipient file"
        hidden
        disabled={disabled || busy}
        onChange={(e) => {
          const source = e.target.files?.[0];
          if (source) void load(source);
        }}
      />
      <button
        type="button"
        className="secondary"
        disabled={disabled || busy}
        onClick={() => input.current?.click()}
      >
        {busy ? "Reading file…" : "Choose recipient file"}
      </button>
      <p className="muted">
        CSV, TXT or Excel (.xlsx), up to 100 addresses. Use one address per
        line, or an Email column. The first Excel sheet is used. This file is
        not attached to the email.
      </p>
      {file && <p>{file.name}</p>}
      {!!recipients.length && (
        <>
          <label>
            Search recipients
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              disabled={disabled || busy}
            />
          </label>
          <div className="button-row">
            <button
              type="button"
              className="secondary"
              disabled={disabled || busy}
              onClick={() => {
                setError("");
                onChange(sourceFile, recipients.join(", "));
              }}
            >
              Select all
            </button>
            <button
              type="button"
              className="secondary"
              disabled={disabled || busy}
              onClick={() => {
                setError("");
                onChange(sourceFile, "");
              }}
            >
              Clear selection
            </button>
            <span role="status">
              {checked.size} of {recipients.length} selected
            </span>
          </div>
          <div
            className="recipient-options"
            role="group"
            aria-label="Recipient selection"
          >
            {recipients
              .filter((v) => v.toLowerCase().includes(query.toLowerCase()))
              .map((address) => (
                <label className="checkbox-label" key={address}>
                  <input
                    type="checkbox"
                    checked={checked.has(address)}
                    disabled={disabled || busy}
                    onChange={(e) => {
                      setError("");
                      const next = new Set(checked);
                      if (e.target.checked) next.add(address);
                      else next.delete(address);
                      onChange(
                        sourceFile,
                        recipients.filter((v) => next.has(v)).join(", "),
                      );
                    }}
                  />
                  {address}
                </label>
              ))}
          </div>
        </>
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
