import { CheckCircle, X } from "lucide-react";

interface SettingsToastProps {
  open: boolean;
  type: "success" | "error";
  message: string;
  onDismiss: () => void;
}

export function SettingsToast({
  open,
  type,
  message,
  onDismiss,
}: SettingsToastProps) {
  if (!open) {
    return null;
  }

  return (
    <div className={`fixed bottom-8 right-8 px-6 py-4 rounded-xl shadow-2xl flex items-center gap-3 animate-in slide-in-from-right-10 z-50 ${type === "success" ? "bg-emerald-600 text-white" : "bg-red-600 text-white"}`}>
      {type === "success" ? <CheckCircle className="w-6 h-6" /> : <X className="w-6 h-6" />}
      <span className="font-bold break-words max-w-[28rem]">{message}</span>
      <button onClick={onDismiss} className="ml-2 text-white/90 hover:text-white transition-colors">
        <X className="w-5 h-5" />
      </button>
    </div>
  );
}
