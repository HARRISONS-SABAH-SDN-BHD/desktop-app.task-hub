import { useId } from "react";
import type { Recipient } from "../domain/recipients";
export function RecipientField({
  label,
  value,
  onChange,
  recipients,
  multiple = false,
  appendSelection = false,
  required = true,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  recipients: Recipient[];
  multiple?: boolean;
  appendSelection?: boolean;
  required?: boolean;
}) {
  const id = useId();
  return (
    <div className="recipient-field">
      <label>
        {label}
        <input
          aria-label={label}
          type="email"
          multiple={multiple}
          required={required}
          maxLength={multiple ? 8000 : 254}
          list={id}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="Choose a saved address or type an email"
        />
      </label>
      <datalist id={id}>
        {recipients.map((contact) => (
          <option
            key={contact.email}
            value={contact.email}
            label={contact.name || contact.email}
          />
        ))}
      </datalist>
      {recipients.length > 0 && (
        <label className="saved-recipient-select">
          Choose from Recipients
          <select
            aria-label={`Choose saved ${label.toLowerCase()}`}
            value=""
            onChange={(e) => {
              if (e.target.value) {
                const current = value
                  .split(",")
                  .map((v) => v.trim())
                  .filter(Boolean);
                onChange(
                  multiple && appendSelection
                    ? [
                        ...current.filter(
                          (v) =>
                            v.toLowerCase() !== e.target.value.toLowerCase(),
                        ),
                        e.target.value,
                      ].join(", ")
                    : e.target.value,
                );
              }
            }}
          >
            <option value="">Choose a saved address…</option>
            {recipients.map((c) => (
              <option key={c.email} value={c.email}>
                {c.name ? `${c.name} — ${c.email}` : c.email}
              </option>
            ))}
          </select>
        </label>
      )}
      <small className="muted">
        Choose an address from Recipients or type one.
        {multiple ? " Separate multiple addresses with commas." : ""}
      </small>
    </div>
  );
}
