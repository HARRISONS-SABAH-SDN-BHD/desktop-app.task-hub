import { useState } from "react";
import { ExcelFlowDialog } from "./ExcelFlowDialog";
export function ExcelFlowAgencies({
  initial,
  locked,
  onClose,
  onSave,
}: {
  initial: [string, string][];
  locked: boolean;
  onClose: () => void;
  onSave: (mapping: Record<string, string>) => void;
}) {
  const [mapping, setMapping] = useState(() => Object.fromEntries(initial));
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const rows = Object.entries(mapping)
    .sort(([a], [b]) => Number(a) - Number(b))
    .filter(([k, v]) =>
      `${k} ${v}`.toLowerCase().includes(query.trim().toLowerCase()),
    );
  const canonical = (value: Record<string, string>) =>
    JSON.stringify(Object.entries(value).sort());
  function close() {
    if (
      canonical(mapping) === canonical(Object.fromEntries(initial)) ||
      window.confirm("Close without saving your IG changes?")
    )
      onClose();
  }
  function clear() {
    setSelected(null);
    setCode("");
    setName("");
    setError("");
  }
  function change(update: boolean) {
    setError("");
    const k = code.trim(),
      v = name.trim();
    if (!/^[1-9]\d{0,7}$/.test(k)) {
      setError(
        "Enter a positive whole IG number (up to 8 digits, no leading zeros).",
      );
      return;
    }
    if (!v) {
      setError("Enter an IG / agency name.");
      return;
    }
    if (update && (!selected || !Object.hasOwn(mapping, selected))) {
      setError("Select a mapping from the list first.");
      return;
    }
    if (Object.hasOwn(mapping, k) && (!update || k !== selected)) {
      setError(
        "This IG number already exists. Select it and use Update selected.",
      );
      return;
    }
    setMapping((current) => {
      const next = { ...current };
      if (update && selected) delete next[selected];
      next[k] = v;
      return next;
    });
    setCode(k);
    setName(v);
    setSelected(k);
  }
  return (
    <ExcelFlowDialog
      title="IG numbers & agency names"
      className="excel-agency-dialog"
      closeButton={false}
      locked={locked}
      onClose={close}
    >
      <fieldset disabled={locked} className="excel-agency-body">
        <div className="excel-agency-search">
          <label htmlFor="ig-search">Search</label>
          <input
            id="ig-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <span>
            {rows.length} / {Object.keys(mapping).length} mappings
          </span>
        </div>
        <div className="excel-agency-table">
          <table>
            <thead>
              <tr>
                <th>IG number</th>
                <th>IG / agency name</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(([k, v]) => (
                <tr key={k} aria-selected={selected === k}>
                  <td>
                    <button
                      className="back-link"
                      type="button"
                      onClick={() => {
                        setSelected(k);
                        setCode(k);
                        setName(v);
                        setError("");
                      }}
                    >
                      {k}
                    </button>
                  </td>
                  <td>
                    <button
                      className="back-link"
                      type="button"
                      onClick={() => {
                        setSelected(k);
                        setCode(k);
                        setName(v);
                        setError("");
                      }}
                    >
                      {v}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <fieldset className="excel-agency-fields">
          <legend>Add or edit a mapping</legend>
          <label>
            IG number
            <input value={code} onChange={(e) => setCode(e.target.value)} />
          </label>
          <label>
            IG / agency name
            <input value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <div className="excel-agency-actions">
            <button className="secondary" onClick={clear}>
              New / clear
            </button>
            <button className="secondary" onClick={() => change(false)}>
              Add
            </button>
            <button className="secondary" onClick={() => change(true)}>
              Update selected
            </button>
            <button
              className="secondary"
              onClick={() => {
                if (!selected) {
                  setError("Select a mapping to remove.");
                  return;
                }
                setMapping((current) => {
                  const next = { ...current };
                  delete next[selected];
                  return next;
                });
                clear();
              }}
            >
              Remove selected
            </button>
          </div>
        </fieldset>
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <p>
          Changes apply to the next report run. Removing a mapping keeps its
          transactions;
          <br />
          the agency label will show the IG number as unmapped.
        </p>
        <div className="excel-dialog-footer">
          <button className="secondary" onClick={close}>
            Cancel
          </button>
          <button className="primary" onClick={() => onSave(mapping)}>
            Save mappings
          </button>
        </div>
      </fieldset>
    </ExcelFlowDialog>
  );
}
