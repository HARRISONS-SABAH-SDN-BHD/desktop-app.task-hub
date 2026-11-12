import { useState, type FormEvent } from "react";
import type { Recipient } from "../domain/recipients";
import type { RecipientUserGroup } from "../domain/recipientUserGroups";
import { userGroups } from "../services/recipientUserGroups";
export function RecipientUserGroups({
  groups,
  recipients,
  refresh,
  busy,
}: {
  groups: RecipientUserGroup[];
  recipients: Recipient[];
  refresh: () => Promise<void>;
  busy: boolean;
}) {
  const [draft, setDraft] = useState<RecipientUserGroup | null>(null);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  async function save(e: FormEvent) {
    e.preventDefault();
    if (!draft) return;
    setError("");
    setPending(true);
    try {
      await userGroups.save(draft.id || null, draft.name, draft.members);
      await refresh();
      setDraft(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }
  const contacts = [...recipients];
  for (const email of draft?.members ?? [])
    if (!contacts.some((c) => c.email === email))
      contacts.push({
        email,
        name: "Previously selected member",
        source: "manual",
      });
  return (
    <section className="email-panel" aria-label="Recipient groups">
      <div className="recipient-list-heading">
        <h2>User groups</h2>
        <button
          type="button"
          className="secondary"
          disabled={busy || pending}
          onClick={() => {
            setDraft({ id: "", name: "", members: [] });
            setQuery("");
            setError("");
          }}
        >
          Create group
        </button>
      </div>
      <p className="muted">
        Choose a saved group for To or CC. Applying it copies its members into
        the routine; later group edits do not change saved routines.
      </p>
      {draft && (
        <form onSubmit={save}>
          <fieldset disabled={busy || pending} className="email-fields">
            <label>
              Group name
              <input
                aria-label="Group name"
                required
                maxLength={100}
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              />
            </label>
            <label>
              Search members
              <input
                type="search"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
            </label>
            <div
              className="user-group-members"
              role="group"
              aria-label="Group members"
            >
              {contacts
                .filter((c) =>
                  `${c.name} ${c.email}`
                    .toLowerCase()
                    .includes(query.toLowerCase()),
                )
                .map((c) => (
                  <label key={c.email}>
                    <input
                      type="checkbox"
                      checked={draft.members.includes(c.email)}
                      onChange={(e) =>
                        setDraft({
                          ...draft,
                          members: e.target.checked
                            ? [...draft.members, c.email]
                            : draft.members.filter((m) => m !== c.email),
                        })
                      }
                    />
                    <span>
                      {c.name ? `${c.name} - ` : ""}
                      {c.email}
                    </span>
                  </label>
                ))}
            </div>
            <p>{draft.members.length} selected (maximum 100)</p>
            <div className="button-row">
              <button className="primary" type="submit">
                Save group
              </button>
              <button
                className="secondary"
                type="button"
                onClick={() => setDraft(null)}
              >
                Cancel group
              </button>
            </div>
          </fieldset>
        </form>
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="recipient-directory">
        {groups.map((group) => (
          <article className="recipient-card" key={group.id}>
            <div>
              <strong>{group.name}</strong>
              <small>{group.members.length} member(s)</small>
              <details>
                <summary>Members</summary>
                {group.members.map((email) => (
                  <p key={email}>{email}</p>
                ))}
              </details>
            </div>
            <div className="button-row">
              <button
                type="button"
                className="secondary"
                disabled={busy || pending}
                aria-label={`Edit group ${group.name}`}
                onClick={() => {
                  setDraft({ ...group, members: [...group.members] });
                  setQuery("");
                  setError("");
                }}
              >
                Edit
              </button>
              <button
                type="button"
                className="secondary"
                disabled={busy || pending}
                aria-label={`Delete group ${group.name}`}
                onClick={async () => {
                  setPending(true);
                  try {
                    await userGroups.remove(group.id);
                    await refresh();
                    if (draft?.id === group.id) setDraft(null);
                  } catch (e) {
                    setError(String(e));
                  } finally {
                    setPending(false);
                  }
                }}
              >
                Delete
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
