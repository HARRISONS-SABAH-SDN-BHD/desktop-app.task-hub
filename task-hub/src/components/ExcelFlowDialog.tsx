import { useEffect, useRef, type ReactNode } from "react";
export function ExcelFlowDialog({
  title,
  children,
  onClose,
  locked = false,
  closeButton = true,
  className = "",
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  locked?: boolean;
  closeButton?: boolean;
  className?: string;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const el = dialog.current!;
    el.showModal();
    return () => el.close();
  }, []);
  return (
    <dialog
      ref={dialog}
      className={`excel-dialog ${className}`}
      aria-label={title}
      onCancel={(event) => {
        event.preventDefault();
        if (!locked) onClose();
      }}
    >
      <header>
        <h2>{title}</h2>
        {closeButton && (
          <button
            className="secondary"
            type="button"
            disabled={locked}
            onClick={onClose}
          >
            Done
          </button>
        )}
      </header>
      {children}
    </dialog>
  );
}
