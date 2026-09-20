import type React from "react";
import { useState } from "react";
import { Modal } from "../common/Modal";

interface CreateGroupModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreateGroup: (name: string, currency: string, participants: string[]) => Promise<void>;
}

interface ParticipantField {
  id: string;
  name: string;
}

export const CreateGroupModal: React.FC<CreateGroupModalProps> = ({
  isOpen,
  onClose,
  onCreateGroup,
}) => {
  const [name, setName] = useState("");
  const [currency, setCurrency] = useState("EUR");
  const [participants, setParticipants] = useState<ParticipantField[]>([
    { id: "p-init-1", name: "" },
    { id: "p-init-2", name: "" },
    { id: "p-init-3", name: "" },
  ]);
  const [submitting, setSubmitting] = useState(false);

  const handleAddParticipantField = () => {
    setParticipants([...participants, { id: `p-${Date.now()}-${Math.random()}`, name: "" }]);
  };

  const handleParticipantChange = (id: string, value: string) => {
    setParticipants(participants.map((p) => (p.id === id ? { ...p, name: value } : p)));
  };

  const handleRemoveParticipantField = (id: string) => {
    if (participants.length <= 1) return;
    setParticipants(participants.filter((p) => p.id !== id));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedName = name.trim();
    const validParticipants = participants.map((p) => p.name.trim()).filter((p) => p.length > 0);

    if (!trimmedName) {
      alert("Please enter a group name");
      return;
    }
    if (validParticipants.length === 0) {
      alert("Please enter at least one participant");
      return;
    }

    setSubmitting(true);
    try {
      await onCreateGroup(trimmedName, currency, validParticipants);
      setName("");
      setCurrency("EUR");
      setParticipants([
        { id: "p-init-1", name: "" },
        { id: "p-init-2", name: "" },
        { id: "p-init-3", name: "" },
      ]);
      onClose();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to create group");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Create New Group" icon="📁">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label
            htmlFor="input-group-name"
            className="block text-xs font-semibold text-slate-300 mb-1.5"
          >
            Group Name *
          </label>
          <input
            id="input-group-name"
            type="text"
            required
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Summer Vacation, Roommates, Dinner Party"
            className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition"
          />
        </div>

        <div>
          <label
            htmlFor="select-group-currency"
            className="block text-xs font-semibold text-slate-300 mb-1.5"
          >
            Currency
          </label>
          <select
            id="select-group-currency"
            value={currency}
            onChange={(e) => setCurrency(e.target.value)}
            className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 focus:outline-none focus:border-indigo-500 transition cursor-pointer"
          >
            <option value="EUR">EUR (€) - Euro</option>
            <option value="USD">USD ($) - US Dollar</option>
            <option value="GBP">GBP (£) - British Pound</option>
            <option value="CHF">CHF - Swiss Franc</option>
            <option value="CAD">CAD (CA$) - Canadian Dollar</option>
          </select>
        </div>

        <div>
          <div className="flex items-center justify-between mb-1.5">
            <span className="block text-xs font-semibold text-slate-300">Initial Participants</span>
            <button
              type="button"
              onClick={handleAddParticipantField}
              className="text-xs text-indigo-400 hover:text-indigo-300 font-semibold transition cursor-pointer"
            >
              + Add Person
            </button>
          </div>

          <div className="space-y-2 max-h-48 overflow-y-auto pr-1">
            {participants.map((p, idx) => (
              <div key={p.id} className="flex items-center gap-2">
                <input
                  type="text"
                  value={p.name}
                  onChange={(e) => handleParticipantChange(p.id, e.target.value)}
                  placeholder={`Participant ${idx + 1}`}
                  className="w-full px-3.5 py-2 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition"
                />
                {participants.length > 1 && (
                  <button
                    type="button"
                    onClick={() => handleRemoveParticipantField(p.id)}
                    className="p-2 text-slate-500 hover:text-rose-400 transition cursor-pointer"
                    title="Remove"
                  >
                    ✕
                  </button>
                )}
              </div>
            ))}
          </div>
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
            {submitting ? "Creating..." : "Create Group"}
          </button>
        </div>
      </form>
    </Modal>
  );
};
