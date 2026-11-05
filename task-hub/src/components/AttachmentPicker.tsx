import { useRef, useState } from "react";
import { Paperclip, X } from "lucide-react";
import { bridge } from "../services/bridge";
import { attachmentService } from "../services/attachments";
import {
  attachmentSize,
  mergeAttachments,
  type AttachmentRef,
} from "../domain/attachments";

export function AttachmentPicker({
  value,
  onChange,
  disabled = false,
  onBusyChange,
  title = "Attachments",
  ariaLabel = "Email attachments",
  maxFiles = 10,
}: {
  value: AttachmentRef[];
  title?: string;
  ariaLabel?: string;
  maxFiles?: number;
  onChange?: (items: AttachmentRef[]) => void;
  disabled?: boolean;
  onBusyChange?: (pending: boolean) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  async function add(select: () => Promise<AttachmentRef[]>) {
    setError("");
    setPending(true);
    onBusyChange?.(true);
    try {
      const next = mergeAttachments(value, await select());
      if (next.length > maxFiles)
        throw new Error(
          `Choose at most ${maxFiles} file${maxFiles === 1 ? "" : "s"}.`,
        );
      onChange?.(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
      onBusyChange?.(false);
    }
  }
  return (
    <section className="attachment-picker" aria-label={ariaLabel}>
      <div className="attachment-heading">
        <strong>
          <Paperclip size={16} /> {title}
        </strong>
        {onChange && (
          <button
            className="secondary"
            type="button"
            disabled={disabled || pending}
            onClick={() =>
              bridge.native
                ? void add(attachmentService.pick)
                : input.current?.click()
            }
          >
            {pending ? "Adding files…" : "Add files"}
          </button>
        )}
      </div>
      {!bridge.native && onChange && (
        <input
          ref={input}
          type="file"
          multiple
          hidden
          aria-label="Attachment files"
          disabled={disabled || pending}
          onChange={(e) => {
            const files = Array.from(e.target.files ?? []);
            e.target.value = "";
            void add(() => attachmentService.preview(files));
          }}
        />
      )}
      {value.length ? (
        <ul className="attachment-list">
          {value.map((item) => (
            <li key={item.id}>
              <span>
                {item.name}
                <small>{attachmentSize(item.size)}</small>
              </span>
              {onChange && (
                <button
                  type="button"
                  className="attachment-remove"
                  aria-label={`Remove ${item.name}`}
                  disabled={disabled || pending}
                  onClick={() => {
                    onChange(value.filter((a) => a.id !== item.id));
                    setError("");
                  }}
                >
                  <X size={16} />
                </button>
              )}
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted">No files attached.</p>
      )}
      {onChange && (
        <p className="email-explanation">
          Up to {maxFiles} {maxFiles === 1 ? "file" : "files"}, 18 MB combined.
          Task Hub stores a copy when added. Replace the attachment to use newer
          contents.
          {!bridge.native &&
            " Browser preview keeps file details only and sends nothing."}
        </p>
      )}
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
    </section>
  );
}
