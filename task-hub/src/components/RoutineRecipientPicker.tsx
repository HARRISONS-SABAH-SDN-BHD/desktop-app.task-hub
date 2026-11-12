import {
  addGroupAddresses,
  groupFitsSender,
  type RecipientUserGroup,
} from "../domain/recipientUserGroups";
import { useEffect, useRef, useState } from "react";
import type { Recipient } from "../domain/recipients";
type Addresses = { recipient: string; cc: string };
type Role = keyof Addresses;
const roles: { key: Role; title: string }[] = [
  { key: "recipient", title: "To" },
  { key: "cc", title: "CC" },
];
const split = (value: string) =>
  value
    .split(",")
    .map((v) => v.trim())
    .filter(Boolean);
export function RoutineRecipientPicker({
  value,
  recipients,
  groups,
  onChange,
}: {
  value: Addresses;
  recipients: Recipient[];
  groups: RecipientUserGroup[];
  onChange: (value: Addresses) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <section
      className="routine-address-summary"
      aria-label="Recipient selection"
    >
      <strong>Recipient</strong>
      <button type="button" className="secondary" onClick={() => setOpen(true)}>
        Choose Recipient
      </button>
      {roles.map((role) => (
        <p key={role.key}>
          <strong>{role.title}:</strong>{" "}
          {value[role.key] ||
            (role.key === "recipient" ? "No recipients selected" : "None")}
        </p>
      ))}
      {open && (
        <RecipientDialog
          value={value}
          recipients={recipients}
          groups={groups}
          onCancel={() => setOpen(false)}
          onApply={(next) => {
            onChange(next);
            setOpen(false);
          }}
        />
      )}
    </section>
  );
}
function RecipientDialog({
  value,
  recipients,
  groups,
  onCancel,
  onApply,
}: {
  value: Addresses;
  recipients: Recipient[];
  groups: RecipientUserGroup[];
  onCancel: () => void;
  onApply: (value: Addresses) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");
  const [draft, setDraft] = useState<Record<Role, string[]>>(() => ({
    recipient: split(value.recipient),
    cc: split(value.cc),
  }));
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    return () => element.close();
  }, []);
  const rows = recipients.map((c) => ({
    email: c.email,
    name: c.name,
    previous: false,
  }));
  for (const address of roles.flatMap((r) => split(value[r.key])))
    if (!rows.some((c) => c.email.toLowerCase() === address.toLowerCase()))
      rows.push({
        email: address,
        name: "Previously selected address",
        previous: true,
      });
  const filtered = rows.filter((c) =>
    `${c.name} ${c.email}`.toLowerCase().includes(query.toLowerCase()),
  );
  function select(address: string, role: Role, checked: boolean) {
    setError("");
    setDraft((current) => {
      const next = { ...current };
      for (const item of roles)
        next[item.key] = current[item.key].filter(
          (v) => v.toLowerCase() !== address.toLowerCase(),
        );
      if (checked) next[role] = [...next[role], address];
      return next;
    });
  }
  return (
    <dialog
      ref={dialog}
      className="routine-recipient-dialog"
      aria-label="Choose recipients"
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" && e.target instanceof HTMLInputElement)
          e.preventDefault();
      }}
    >
      <header>
        <h2>Choose recipients</h2>
        <button type="button" className="secondary" onClick={onCancel}>
          Cancel
        </button>
      </header>
      <p>
        Select one or more addresses for To or CC. Choose one role per address.
      </p>
      {!!groups.length && (
        <section className="popup-user-groups" aria-label="Choose user group">
          <h3>User groups</h3>
          <p>
            Groups are available when all members belong to this sender's saved
            list.
          </p>
          {groups.map((group) => (
            <div className="recipient-card" key={group.id}>
              <span>
                <strong>{group.name}</strong> ({group.members.length})
                {!groupFitsSender(group, recipients) && (
                  <small>
                    Some members are missing from this sender's list.
                  </small>
                )}
              </span>
              <div className="button-row">
                {roles.map((role) => (
                  <button
                    key={role.key}
                    className="secondary"
                    type="button"
                    disabled={!groupFitsSender(group, recipients)}
                    aria-label={`Add ${group.name} to ${role.title}`}
                    onClick={() => {
                      try {
                        setDraft(
                          addGroupAddresses(draft, group.members, role.key),
                        );
                        setError("");
                      } catch (e) {
                        setError(String(e));
                      }
                    }}
                  >
                    {role.title}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </section>
      )}
      <label className="recipient-search">
        Search emails
        <input
          autoFocus
          type="search"
          aria-label="Search recipient emails"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>
      <div className="recipient-modal-scroll">
        <table>
          <thead>
            <tr>
              <th scope="col">Email</th>
              {roles.map((role) => (
                <th scope="col" key={role.key}>
                  {role.title}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {filtered.map((contact) => (
              <tr key={contact.email}>
                <td>
                  {contact.name && <strong>{contact.name}</strong>}
                  <span>{contact.email}</span>
                  {contact.previous && (
                    <small>
                      Not in this sender's current list. Keep or remove this
                      saved selection.
                    </small>
                  )}
                </td>
                {roles.map((role) => (
                  <td key={role.key}>
                    <input
                      type="checkbox"
                      aria-label={`${role.title} ${contact.email}`}
                      checked={draft[role.key].some(
                        (a) => a.toLowerCase() === contact.email.toLowerCase(),
                      )}
                      onChange={(e) =>
                        select(contact.email, role.key, e.target.checked)
                      }
                    />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        {!filtered.length && (
          <p>
            {rows.length
              ? "No matching emails."
              : "No saved recipients for this sender. Scan or add contacts on the Recipients page first."}
          </p>
        )}
      </div>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <footer>
        <span>
          {roles
            .map((role) => `${role.title}: ${draft[role.key].length}`)
            .join(" · ")}
        </span>
        <button
          type="button"
          className="primary"
          onClick={() => {
            if (!draft.recipient.length) {
              setError("Select at least one To recipient.");
              return;
            }
            onApply({
              recipient: draft.recipient.join(", "),
              cc: draft.cc.join(", "),
            });
          }}
        >
          Apply recipients
        </button>
      </footer>
    </dialog>
  );
}
