import type React from "react";
import { useEffect, useState } from "react";
import { Modal } from "../common/Modal";

interface JoinGroupModalProps {
  isOpen: boolean;
  onClose: () => void;
  onJoinGroup: (inviteCode: string) => Promise<void>;
}

export const JoinGroupModal: React.FC<JoinGroupModalProps> = ({ isOpen, onClose, onJoinGroup }) => {
  const [code, setCode] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setCode("");
    setError(null);
  }, [isOpen]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!code.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      await onJoinGroup(code.trim());
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Join a Group" icon="📥">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label
            htmlFor="input-invite-code"
            className="block text-xs font-semibold text-slate-300 mb-1.5"
          >
            Invite code *
          </label>
          <textarea
            id="input-invite-code"
            required
            rows={3}
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="ezcount://join?…"
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-[11px] font-mono text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition break-all resize-none"
          />
          <p className="text-[11px] text-slate-500 mt-1.5">
            Ask a member to open the group, tap Share and copy the invite code.
          </p>
        </div>
        {error && (
          <p role="alert" className="text-xs text-rose-300">
            {error}
          </p>
        )}
        <div className="flex items-center justify-end gap-2.5 pt-3 border-t border-slate-800">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 rounded-xl text-xs font-semibold text-slate-400 hover:text-slate-200 transition cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={submitting}
            className="px-5 py-2 rounded-xl text-xs font-semibold bg-indigo-600 hover:bg-indigo-500 text-white shadow-md shadow-indigo-600/20 transition cursor-pointer disabled:opacity-50"
          >
            {submitting ? "Joining…" : "Join Group"}
          </button>
        </div>
      </form>
    </Modal>
  );
};
