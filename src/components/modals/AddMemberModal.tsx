import type React from "react";
import { useState } from "react";
import { Modal } from "../common/Modal";

interface AddMemberModalProps {
  isOpen: boolean;
  onClose: () => void;
  onAddMember: (name: string) => Promise<void>;
}

export const AddMemberModal: React.FC<AddMemberModalProps> = ({ isOpen, onClose, onAddMember }) => {
  const [name, setName] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;

    setSubmitting(true);
    try {
      await onAddMember(trimmed);
      setName("");
      onClose();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to add member");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Add Group Member" icon="👤">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label
            htmlFor="input-member-name"
            className="block text-xs font-semibold text-slate-300 mb-1.5"
          >
            Member Name *
          </label>
          <input
            id="input-member-name"
            type="text"
            required
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. David, Sarah"
            className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition"
          />
        </div>

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
            {submitting ? "Adding..." : "Add to Group"}
          </button>
        </div>
      </form>
    </Modal>
  );
};
