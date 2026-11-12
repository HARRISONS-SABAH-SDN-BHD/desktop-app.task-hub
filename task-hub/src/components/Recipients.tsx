import { RecipientUserGroups } from "./RecipientUserGroups";
import type { RecipientUserGroup } from "../domain/recipientUserGroups";
import { useState, type FormEvent } from "react";
import { Help } from "./Help";
import { recipientBook } from "../services/recipients";
import {
  ALL_ACCOUNTS,
  UNKNOWN_ACCOUNT,
  filterRecipientGroup,
  hasUnknownAccount,
  recipientAccounts,
  recipientAccountChoices,
} from "../domain/recipientGroups";
import type { RecipientProvider } from "../domain/recipients";
import type { Recipient } from "../domain/recipients";
export function Recipients({
  recipients,
  groups,
  refresh,
  gmailEmail,
  mailserverEmail,
  native,
  busy,
}: {
  recipients: Recipient[];
  groups: RecipientUserGroup[];
  refresh: () => Promise<void>;
  gmailEmail: string | null;
  mailserverEmail: string | null;
  native: boolean;
  busy: boolean;
}) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [query, setQuery] = useState("");
  const [choice, setChoice] = useState("");
  const [tab, setTab] = useState<RecipientProvider>(
    gmailEmail || !mailserverEmail ? "gmail" : "mailserver",
  );
  const [groupAccount, setGroupAccount] = useState<
    Record<RecipientProvider, string>
  >({ gmail: ALL_ACCOUNTS, mailserver: ALL_ACCOUNTS });
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const accounts = [
    ...(gmailEmail
      ? [{ id: `gmail:${gmailEmail}`, provider: "gmail", email: gmailEmail }]
      : []),
    ...(mailserverEmail
      ? [
          {
            id: `mailserver:${mailserverEmail}`,
            provider: "mailserver",
            email: mailserverEmail,
          },
        ]
      : []),
  ];
  const account = accounts.find((a) => a.id === choice) ?? accounts[0];
  const locked = busy || pending;
  async function act(work: () => Promise<string>) {
    setPending(true);
    setError("");
    setNotice("");
    try {
      const message = await work();
      await refresh();
      setNotice(message);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setPending(false);
    }
  }
  function add(event: FormEvent) {
    event.preventDefault();
    void act(async () => {
      await recipientBook.save(email, name);
      setEmail("");
      setName("");
      return "Recipient saved.";
    });
  }
  const connected = tab === "gmail" ? gmailEmail : mailserverEmail;
  const providerTitle = tab === "gmail" ? "Gmail" : "IMAP / POP3";
  const choices = recipientAccountChoices(recipients, tab, connected);
  const unknown = recipients.some((c) => hasUnknownAccount(c, tab));
  const selected = groupAccount[tab];
  const selectedAccount =
    selected === ALL_ACCOUNTS ||
    (selected === UNKNOWN_ACCOUNT && unknown) ||
    choices.includes(selected)
      ? selected
      : ALL_ACCOUNTS;
  const scoped = filterRecipientGroup(recipients, tab, selectedAccount);
  const filtered = scoped.filter((c) =>
    `${c.name} ${c.email}`.toLowerCase().includes(query.toLowerCase()),
  );
  function sourceLabel(contact: Recipient): string {
    if (contact.source === "manual")
      return "Added manually · available in both tabs";
    const origins = recipientAccounts(contact, tab);
    return `Scanned from ${providerTitle} · ${origins.length ? origins.join(", ") : "earlier scan (account unknown)"}`;
  }
  return (
    <section aria-label="Recipients" className="recipients-page">
      <h1>Recipients</h1>
      <section className="email-panel">
        <h2>Scan email addresses</h2>
        <Help label="Scanning recipients">
          Reads From, To, CC and Reply-To headers from up to 50 recent messages.
          Gmail includes received and sent messages; IMAP uses your configured
          folder; POP3 only sees messages still on the server. Gmail needs Read
          email addresses and attachments enabled in Connect. This does not read
          message bodies or attachments, send email, or change routines.
          Scanning again can restore deleted addresses. Account filters include
          earlier scans; reconnect an earlier account in Connect to scan it
          again. Manually added recipients are shared between both tabs.
        </Help>
        <div className="email-fields">
          <label>
            Email account
            <select
              aria-label="Scan email account"
              value={account?.id ?? ""}
              disabled={locked || !accounts.length}
              onChange={(e) => {
                setChoice(e.target.value);
                const selected = accounts.find((a) => a.id === e.target.value);
                if (selected) {
                  const provider = selected.provider as RecipientProvider;
                  setTab(provider);
                  setGroupAccount((current) => ({
                    ...current,
                    [provider]: selected.email.toLowerCase(),
                  }));
                }
              }}
            >
              <option value="" disabled>
                Choose a connected account
              </option>
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.provider === "gmail" ? "Gmail" : "IMAP / POP3"} — {a.email}
                </option>
              ))}
            </select>
          </label>
          <button
            className="secondary"
            type="button"
            disabled={locked || !native || !account}
            onClick={() => {
              if (account)
                void act(async () => {
                  const added = await recipientBook.scan(
                    account.provider,
                    account.email,
                  );
                  const provider = account.provider as RecipientProvider;
                  setTab(provider);
                  setGroupAccount((current) => ({
                    ...current,
                    [provider]: account.email.toLowerCase(),
                  }));
                  return `Scan finished. Added ${added} recipient(s) to this account.`;
                });
            }}
          >
            {pending ? "Working…" : "Scan emails"}
          </button>
          {!native && (
            <p className="muted">
              Email scanning is available in the desktop app. You can add
              recipients manually in this preview.
            </p>
          )}
          {native && !account && (
            <p className="muted">
              Connect an email account in Connect to scan its addresses.
            </p>
          )}
        </div>
      </section>
      <form className="email-panel" onSubmit={add}>
        <h2>Add recipient</h2>
        <fieldset className="email-fields" disabled={locked}>
          <div className="routine-form-grid">
            <label>
              Name (optional)
              <input
                aria-label="Recipient name"
                value={name}
                maxLength={100}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <label>
              Email address
              <input
                aria-label="Recipient email address"
                type="email"
                required
                value={email}
                maxLength={254}
                onChange={(e) => setEmail(e.target.value)}
              />
            </label>
          </div>
          <button type="submit" className="primary">
            Add recipient
          </button>
        </fieldset>
      </form>
      <RecipientUserGroups
        groups={groups}
        recipients={recipients}
        refresh={refresh}
        busy={locked}
      />
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
      <section className="email-panel">
        <div className="recipient-list-heading">
          <h2>Saved recipients ({scoped.length})</h2>
          <div
            className="recipient-provider-tabs"
            role="tablist"
            aria-label="Recipient provider"
          >
            <button
              type="button"
              role="tab"
              id="recipient-gmail-tab"
              aria-controls="saved-recipient-list"
              aria-selected={tab === "gmail"}
              className={tab === "gmail" ? "active" : "secondary"}
              onClick={() => {
                setTab("gmail");
                setQuery("");
              }}
            >
              Gmail
            </button>
            <button
              type="button"
              role="tab"
              id="recipient-mailserver-tab"
              aria-controls="saved-recipient-list"
              aria-selected={tab === "mailserver"}
              className={tab === "mailserver" ? "active" : "secondary"}
              onClick={() => {
                setTab("mailserver");
                setQuery("");
              }}
            >
              IMAP / POP3
            </button>
          </div>
        </div>
        <div className="email-fields recipient-account-filter">
          <label>
            {providerTitle} account
            <select
              aria-label={`${providerTitle} recipient account`}
              value={selectedAccount}
              onChange={(e) =>
                setGroupAccount((current) => ({
                  ...current,
                  [tab]: e.target.value,
                }))
              }
            >
              <option value={ALL_ACCOUNTS}>All {providerTitle} accounts</option>
              {choices.map((address) => (
                <option key={address} value={address}>
                  {address}
                  {connected?.toLowerCase() === address
                    ? " (connected)"
                    : " (saved scan)"}
                </option>
              ))}
              {unknown && (
                <option value={UNKNOWN_ACCOUNT}>
                  Earlier scans — account unknown
                </option>
              )}
            </select>
          </label>
        </div>
        <label className="recipient-search">
          Search recipients
          <input
            type="search"
            aria-label="Search saved recipients"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        {!filtered.length && (
          <p className="muted">
            {scoped.length
              ? "No matching recipients."
              : "No saved recipients for this account. Scan emails or add an address above."}
          </p>
        )}
        <div
          className="recipient-directory"
          id="saved-recipient-list"
          role="tabpanel"
          aria-labelledby={
            tab === "gmail" ? "recipient-gmail-tab" : "recipient-mailserver-tab"
          }
        >
          {filtered.map((contact) => (
            <article className="recipient-card" key={contact.email}>
              <div>
                <strong>{contact.name || contact.email}</strong>
                {contact.name && <p>{contact.email}</p>}
                <small className="muted">{sourceLabel(contact)}</small>
              </div>
              <button
                type="button"
                className="secondary"
                disabled={locked}
                aria-label={`Delete ${contact.email}`}
                onClick={() =>
                  void act(async () => {
                    await recipientBook.remove(contact.email);
                    return "Recipient removed from the saved list. Scanning can add it again. Saved routines are unchanged.";
                  })
                }
              >
                Delete
              </button>
            </article>
          ))}
        </div>
      </section>
    </section>
  );
}
