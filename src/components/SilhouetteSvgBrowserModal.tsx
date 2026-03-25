import { useEffect } from "react";
import { createPortal } from "react-dom";
import { SilhouetteSvgBrowser } from "./SilhouetteSvgBrowser";

export type SilhouetteSvgBrowserModalProps = {
  open: boolean;
  onClose: () => void;
};

/**
 * Full-screen-style modal for dev silhouette previews; stacks above the settings dialog.
 */
export function SilhouetteSvgBrowserModal({
  open,
  onClose,
}: SilhouetteSvgBrowserModalProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <div
      className="modal-backdrop svg-browser-modal-backdrop"
      role="presentation"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="modal svg-browser-modal"
        role="dialog"
        aria-labelledby="svg-browser-modal-title"
        aria-modal="true"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="modal-head">
          <h2 id="svg-browser-modal-title">Silhouette SVG preview</h2>
          <button
            type="button"
            className="modal-close"
            onClick={onClose}
            aria-label="Close"
          >
            ×
          </button>
        </div>
        <SilhouetteSvgBrowser />
      </div>
    </div>,
    document.body,
  );
}
