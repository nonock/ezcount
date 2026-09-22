import type React from "react";
import { useEffect } from "react";

interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  icon?: string;
  children: React.ReactNode;
  maxWidthClass?: string;
}

export const Modal: React.FC<ModalProps> = ({
  isOpen,
  onClose,
  title,
  icon,
  children,
  maxWidthClass = "max-w-md",
}) => {
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const titleId = `modal-title-${title.toLowerCase().replace(/[^a-z0-9]/g, "-")}`;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-150">
      <button
        type="button"
        tabIndex={-1}
        aria-label="Dismiss backdrop"
        onClick={onClose}
        className="fixed inset-0 w-full h-full bg-transparent border-0 cursor-default -z-10"
      />
      <div
        aria-modal="true"
        aria-labelledby={titleId}
        className={`relative z-10 w-full ${maxWidthClass} bg-slate-900 border border-slate-800 rounded-2xl shadow-2xl p-5 sm:p-6 space-y-4 max-h-[85vh] overflow-y-auto overscroll-contain touch-manipulation`}
      >
        <div className="flex items-center justify-between border-b border-slate-800 pb-3">
          <div className="flex items-center gap-2.5">
            {icon && (
              <div
                aria-hidden="true"
                className="w-7 h-7 rounded-lg bg-indigo-500/10 border border-indigo-500/20 text-indigo-400 flex items-center justify-center text-xs select-none"
              >
                {icon}
              </div>
            )}
            <h2 id={titleId} className="text-base font-bold text-white tracking-tight">
              {title}
            </h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Dismiss dialog"
            className="text-slate-400 hover:text-white transition-colors duration-150 cursor-pointer p-1.5 rounded-lg hover:bg-slate-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
          >
            <svg
              className="w-4 h-4"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2"
              aria-hidden="true"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        {children}
      </div>
    </div>
  );
};
