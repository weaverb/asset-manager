import { createPortal } from "react-dom";

export type ConfirmModalProps = {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
};

export function ConfirmModal({
  title,
  message,
  confirmLabel = "Delete",
  cancelLabel = "Cancel",
  danger = true,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  return createPortal(
    <div
      className="modal-backdrop confirm-modal-backdrop"
      role="presentation"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div
        className="modal confirm-modal"
        role="alertdialog"
        aria-labelledby="confirm-title"
        aria-describedby="confirm-desc"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <h2 id="confirm-title" className="confirm-modal-title">
          {title}
        </h2>
        <p id="confirm-desc" className="confirm-modal-msg">
          {message}
        </p>
        <div className="modal-actions">
          <button type="button" onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            type="button"
            className={danger ? "primary danger-solid" : "primary"}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
