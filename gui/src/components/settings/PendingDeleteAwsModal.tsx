interface PendingDeleteAwsModalProps {
  open: boolean;
  profileName: string | null;
  onCancel: () => void;
  onConfirm: () => void | Promise<void>;
}

export function PendingDeleteAwsModal({
  open,
  profileName,
  onCancel,
  onConfirm,
}: PendingDeleteAwsModalProps) {
  if (!open || !profileName) {
    return null;
  }

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4">
      <div className="bg-white dark:bg-slate-800 rounded-xl p-6 w-full max-w-md shadow-2xl border border-slate-200 dark:border-slate-700">
        <h3 className="text-xl font-bold text-slate-900 dark:text-white">Remove AWS Profile</h3>
        <p className="text-sm text-slate-600 dark:text-slate-300 mt-2">
          Remove profile <span className="font-semibold">{profileName}</span> from local AWS configuration?
        </p>
        <p className="text-xs text-slate-500 dark:text-slate-400 mt-2">
          This removes the profile from <code>~/.aws/credentials</code> and matching region config.
        </p>
        <div className="mt-6 flex justify-end gap-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-slate-500 dark:text-slate-300 hover:text-slate-700 dark:hover:text-white transition-colors font-medium"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            className="px-4 py-2 rounded-lg bg-red-600 hover:bg-red-700 text-white font-semibold"
          >
            Remove
          </button>
        </div>
      </div>
    </div>
  );
}
