import type React from "react";
import { useEffect, useRef, useState } from "react";
import type { Group } from "../../types";
import { Modal } from "../common/Modal";

interface RecordReimbursementModalProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group;
  initialFromId?: string;
  initialToId?: string;
  initialAmount?: string;
  onRecordReimbursement: (
    fromId: string,
    toId: string,
    amountCents: number,
    notes?: string
  ) => Promise<void>;
}

export const RecordReimbursementModal: React.FC<RecordReimbursementModalProps> = ({
  isOpen,
  onClose,
  group,
  initialFromId,
  initialToId,
  initialAmount,
  onRecordReimbursement,
}) => {
  const [fromId, setFromId] = useState("");
  const [toId, setToId] = useState("");
  const [amountStr, setAmountStr] = useState("");
  const [notes, setNotes] = useState("");
  const [submitting, setSubmitting] = useState(false);

  // Initialize once per opening so a background sync refreshing the group keeps the form intact.
  const initialized = useRef(false);
  useEffect(() => {
    if (!isOpen) {
      initialized.current = false;
      return;
    }
    if (initialized.current || group.participants.length === 0) return;
    initialized.current = true;

    const active = group.participants.filter((p) => !p.removed);
    const pool = active.length > 0 ? active : group.participants;
    const p1 = pool[0].id;
    const p2 = pool.length > 1 ? pool[1].id : p1;

    setFromId(initialFromId || p1);
    setToId(initialToId || (initialFromId === p1 ? p2 : p1));
    setAmountStr(initialAmount || "");
    setNotes("");
  }, [isOpen, group, initialFromId, initialToId, initialAmount]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (fromId === toId) {
      alert("The sender and recipient cannot be the same person.");
      return;
    }

    const amountDecimal = Number.parseFloat(amountStr);
    const amountCents = !Number.isNaN(amountDecimal) ? Math.round(amountDecimal * 100) : 0;

    if (amountCents <= 0) {
      alert("Please enter a valid amount greater than 0");
      return;
    }

    setSubmitting(true);
    try {
      await onRecordReimbursement(fromId, toId, amountCents, notes.trim() || undefined);
      onClose();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to record reimbursement");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Record Reimbursement" icon="🤝">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <div>
            <label
              htmlFor="select-reimburse-from"
              className="block text-xs font-semibold text-slate-300 mb-1.5"
            >
              Who paid? (Sender) *
            </label>
            <select
              id="select-reimburse-from"
              required
              value={fromId}
              onChange={(e) => setFromId(e.target.value)}
              className="w-full px-3 py-2 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 focus:outline-none focus:border-emerald-500 transition cursor-pointer"
            >
              {group.participants.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                  {p.removed ? " (removed)" : ""}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label
              htmlFor="select-reimburse-to"
              className="block text-xs font-semibold text-slate-300 mb-1.5"
            >
              Who received? (Recipient) *
            </label>
            <select
              id="select-reimburse-to"
              required
              value={toId}
              onChange={(e) => setToId(e.target.value)}
              className="w-full px-3 py-2 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 focus:outline-none focus:border-emerald-500 transition cursor-pointer"
            >
              {group.participants.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                  {p.removed ? " (removed)" : ""}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div>
          <label
            htmlFor="input-reimburse-amount"
            className="block text-xs font-semibold text-slate-300 mb-1.5"
          >
            Amount *
          </label>
          <div className="relative">
            <input
              id="input-reimburse-amount"
              type="number"
              step="0.01"
              min="0.01"
              placeholder="0.00"
              required
              value={amountStr}
              onChange={(e) => setAmountStr(e.target.value)}
              className="w-full pl-3.5 pr-14 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm font-semibold text-slate-100 placeholder-slate-500 focus:outline-none focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500 transition"
            />
            <div className="absolute right-3.5 top-1/2 -translate-y-1/2 text-xs font-bold text-slate-400 pointer-events-none">
              {group.currency}
            </div>
          </div>
        </div>

        <div>
          <label
            htmlFor="input-reimburse-notes"
            className="block text-xs font-semibold text-slate-300 mb-1.5"
          >
            Payment Note (Optional)
          </label>
          <input
            id="input-reimburse-notes"
            type="text"
            placeholder="e.g. Revolut, Bank transfer, Cash"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            className="w-full px-3.5 py-2 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-emerald-500 transition"
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
            className="px-5 py-2 rounded-xl text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white shadow-md shadow-emerald-600/20 transition cursor-pointer disabled:opacity-50"
          >
            {submitting ? "Saving..." : "Confirm Payment"}
          </button>
        </div>
      </form>
    </Modal>
  );
};
