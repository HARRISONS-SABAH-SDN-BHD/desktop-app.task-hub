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
}: {
  value: AttachmentRef[];
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
      onChange?.(mergeAttachments(value, await select()));
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
      onBusyChange?.(false);
    }
  }
  return (
    <section className="attachment-picker" aria-label="Email attachments">
      <div className="attachment-heading">
        <strong>
          <Paperclip size={16} /> Attachments
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
          Up to 10 files, 18 MB combined. Task Hub stores a copy when added.
          Replace the attachment to use newer contents.
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
