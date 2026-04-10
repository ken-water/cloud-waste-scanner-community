import { Modal } from "../Modal";

interface ConfirmActionModalProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmLabel: string;
  confirmClassName?: string;
  confirmingAction: boolean;
  onCancel: () => void;
  onConfirm: () => void | Promise<void>;
}

export function ConfirmActionModal({
  isOpen,
  title,
  message,
  confirmLabel,
  confirmClassName,
  confirmingAction,
  onCancel,
  onConfirm,
}: ConfirmActionModalProps) {
  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        if (!confirmingAction) {
          onCancel();
        }
      }}
      title={title}
      footer={
        <div className="flex gap-2">
          <button
            onClick={onCancel}
            disabled={confirmingAction}
            className="px-4 py-2 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 rounded-lg font-medium disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            disabled={confirmingAction}
            className={`px-4 py-2 rounded-lg font-medium disabled:opacity-60 ${confirmClassName || "bg-indigo-600 hover:bg-indigo-700 text-white"}`}
          >
            {confirmingAction ? "Processing..." : confirmLabel}
          </button>
        </div>
      }
    >
      <p className="text-sm text-slate-600 dark:text-slate-300">{message}</p>
    </Modal>
  );
}
