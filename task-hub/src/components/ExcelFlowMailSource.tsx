import { useState } from "react";
import { Help } from "./Help";
import { attachmentSize } from "../domain/attachments";
import {
  excelFlowWorkspace,
  type WorkbookInput,
  type SourceFolder,
  type SourceMessage,
  type SourceAttachment,
} from "../services/excelflowWorkspace";
export function ExcelFlowMailSource({
  native,
  connected,
  locked,
  onConnect,
  onDisc,
  onFtg,
}: {
  native: boolean;
  connected: Record<string, boolean>;
  locked: boolean;
  onConnect: (provider: string) => void;
  onDisc: (input: WorkbookInput) => void;
  onFtg: (input: WorkbookInput) => void;
}) {
  const [provider, setProvider] = useState("gmail");
  const [folders, setFolders] = useState<SourceFolder[]>([]);
  const [folderId, setFolderId] = useState("");
  const [messages, setMessages] = useState<SourceMessage[]>([]);
  const [message, setMessage] = useState<SourceMessage | null>(null);
  const [attachments, setAttachments] = useState<SourceAttachment[]>([]);
  const [search, setSearch] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const folder = folders.find((f) => f.id === folderId);
  async function act(action: () => Promise<void>) {
    setPending(true);
    setError("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }
  function source(value: string) {
    setProvider(value);
    setFolders([]);
    setFolderId("");
    setMessages([]);
    setMessage(null);
    setAttachments([]);
  }
  const setDisc = onDisc,
    setFtg = onFtg;
  locked = locked || pending;
  return (
    <>
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      <section className="email-panel routine-form">
        <h2>Read files from email</h2>
        <div className="routine-form-grid">
          <label>
            Email connection
            <select
              aria-label="Email connection"
              value={provider}
              disabled={locked}
              onChange={(e) => source(e.target.value)}
            >
              <option value="gmail">Gmail</option>
              <option value="mailserver">IMAP / POP3</option>
            </select>
          </label>
        </div>
        {!connected[provider] && (
          <p className="notice">
            Connect this email source first.{" "}
            <button
              type="button"
              className="secondary"
              disabled={locked}
              onClick={() => onConnect(provider)}
            >
              Connect email source
            </button>
          </p>
        )}
        <button
          type="button"
          className="secondary"
          disabled={!native || locked || !connected[provider]}
          onClick={() =>
            void act(async () => {
              const result = await excelFlowWorkspace.folders(provider);
              setFolders(result);
              setFolderId(result[0]?.id ?? "");
              setMessages([]);
              setAttachments([]);
            })
          }
        >
          Read folders
        </button>
        {!!folders.length && (
          <>
            <label className="workspace-select">
              Email folder
              <select
                aria-label="Email folder"
                value={folderId}
                disabled={locked}
                onChange={(e) => {
                  setFolderId(e.target.value);
                  setMessages([]);
                  setAttachments([]);
                  setMessage(null);
                }}
              >
                {folders.map((f) => (
                  <option value={f.id} key={f.id}>
                    {f.name}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              className="secondary"
              disabled={locked || !folder}
              onClick={() =>
                void act(async () => {
                  if (folder)
                    setMessages(
                      await excelFlowWorkspace.messages(provider, folder),
                    );
                  setMessage(null);
                  setAttachments([]);
                })
              }
            >
              Read recent emails
            </button>
          </>
        )}
        <Help label="Reading email safely">
          Only message headers and workbook attachments are read. Task Hub does
          not delete emails or mark them as read. Gmail needs optional read
          permission. Gmail shows the Inbox; IMAP uses its configured incoming
          folder. The list is limited to recent emails.
        </Help>
        {!!messages.length && (
          <>
            <label className="workspace-select">
              Find an email
              <input
                type="search"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Subject or sender"
              />
            </label>
            <div
              className="mail-message-list"
              role="group"
              aria-label="Recent emails"
            >
              {messages
                .filter((m) =>
                  `${m.subject} ${m.sender}`
                    .toLowerCase()
                    .includes(search.toLowerCase()),
                )
                .map((m) => (
                  <button
                    type="button"
                    key={m.id}
                    disabled={locked}
                    aria-pressed={message?.id === m.id}
                    className={message?.id === m.id ? "primary" : "secondary"}
                    onClick={() =>
                      void act(async () => {
                        setMessage(m);
                        setAttachments([]);
                        if (folder)
                          setAttachments(
                            await excelFlowWorkspace.attachments(
                              provider,
                              m,
                              folder,
                            ),
                          );
                      })
                    }
                  >
                    <strong>{m.subject || "(No subject)"}</strong>
                    <span>
                      {m.sender} · {m.received}
                    </span>
                  </button>
                ))}
            </div>
          </>
        )}
        {message && (
          <div>
            <h3>Workbook attachments</h3>
            {!attachments.length && <p>No .xlsx or .xlsm attachments found.</p>}
            {attachments.map((a) => (
              <div className="workbook-attachment" key={a.id}>
                <span>
                  <strong>{a.name}</strong> · {attachmentSize(a.size)}
                </span>
                <button
                  type="button"
                  className="secondary"
                  disabled={locked}
                  onClick={() =>
                    void act(async () => {
                      if (folder)
                        setDisc(
                          await excelFlowWorkspace.download(
                            provider,
                            message,
                            folder,
                            a,
                          ),
                        );
                    })
                  }
                >
                  Use as DISC
                </button>
                <button
                  type="button"
                  className="secondary"
                  disabled={locked}
                  onClick={() =>
                    void act(async () => {
                      if (folder)
                        setFtg(
                          await excelFlowWorkspace.download(
                            provider,
                            message,
                            folder,
                            a,
                          ),
                        );
                    })
                  }
                >
                  Use as FTG
                </button>
              </div>
            ))}
          </div>
        )}
      </section>
    </>
  );
}
