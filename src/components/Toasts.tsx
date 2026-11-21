import { CircleAlert, CircleCheckBig, Info, X } from "lucide-react";
import { useAppStore } from "../store/useAppStore";

const ICON = {
  info: <Info size={16} />,
  success: <CircleCheckBig size={16} />,
  error: <CircleAlert size={16} />,
};

export function Toasts() {
  const toasts = useAppStore((s) => s.toasts);
  const dismiss = useAppStore((s) => s.dismissToast);
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`} role={t.kind === "error" ? "alert" : "status"}>
          {ICON[t.kind]}
          <span>{t.message}</span>
          <button className="toast-close" onClick={() => dismiss(t.id)} aria-label="Dismiss">
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
