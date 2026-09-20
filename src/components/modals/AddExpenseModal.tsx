import type React from "react";
import { useEffect, useState } from "react";
import type { Group } from "../../types";
import { formatMoney } from "../../utils/formatters";
import { Modal } from "../common/Modal";

interface AddExpenseModalProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group;
  onAddExpense: (
    title: string,
    amountCents: number,
    paidBy: string,
    splitAmong: string[]
  ) => Promise<void>;
}

export const AddExpenseModal: React.FC<AddExpenseModalProps> = ({
  isOpen,
  onClose,
  group,
  onAddExpense,
}) => {
  const [title, setTitle] = useState("");
  const [amountStr, setAmountStr] = useState("");
  const [paidBy, setPaidBy] = useState("");
  const [splitAmong, setSplitAmong] = useState<string[]>([]);
  const [submitting, setSubmitting] = useState(false);

  // Initialize defaults when modal opens or group changes
  useEffect(() => {
    if (isOpen && group.participants.length > 0) {
      setPaidBy((prev) =>
        prev && group.participants.some((p) => p.id === prev) ? prev : group.participants[0].id
      );
      setSplitAmong(group.participants.map((p) => p.id));
    }
  }, [isOpen, group]);

  const handleToggleParticipant = (id: string) => {
    if (splitAmong.includes(id)) {
      setSplitAmong(splitAmong.filter((x) => x !== id));
    } else {
      setSplitAmong([...splitAmong, id]);
    }
  };

  const handleToggleAll = () => {
    if (splitAmong.length === group.participants.length) {
      setSplitAmong([]);
    } else {
      setSplitAmong(group.participants.map((p) => p.id));
    }
  };

  const amountDecimal = Number.parseFloat(amountStr);
  const amountCents = !Number.isNaN(amountDecimal) ? Math.round(amountDecimal * 100) : 0;
  const perPersonCents = splitAmong.length > 0 ? Math.floor(amountCents / splitAmong.length) : 0;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedTitle = title.trim();

    if (!trimmedTitle) {
      alert("Please enter an expense description");
      return;
    }
    if (amountCents <= 0) {
      alert("Please enter a valid positive amount");
      return;
    }
    if (splitAmong.length === 0) {
      alert("Please select at least one participant to split the bill with");
      return;
    }

    setSubmitting(true);
    try {
      await onAddExpense(trimmedTitle, amountCents, paidBy, splitAmong);
      setTitle("");
      setAmountStr("");
      onClose();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to add expense");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Add New Expense" icon="🧾">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label
            htmlFor="input-expense-title"
            className="block text-xs font-semibold text-slate-300 mb-1.5"
          >
            Description *
          </label>
          <input
            id="input-expense-title"
            type="text"
            required
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="e.g. Groceries, Dinner, Taxi, Museum tickets"
            className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition"
          />
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <div>
            <label
              htmlFor="input-expense-amount"
              className="block text-xs font-semibold text-slate-300 mb-1.5"
            >
              Amount *
            </label>
            <div className="relative">
              <input
                id="input-expense-amount"
                type="number"
                step="0.01"
                min="0.01"
                required
                value={amountStr}
                onChange={(e) => setAmountStr(e.target.value)}
                placeholder="0.00"
                className="w-full pl-3.5 pr-14 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm font-semibold text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition"
              />
              <div className="absolute right-3.5 top-1/2 -translate-y-1/2 text-xs font-bold text-slate-400 pointer-events-none">
                {group.currency}
              </div>
            </div>
          </div>

          <div>
            <label
              htmlFor="select-expense-payer"
              className="block text-xs font-semibold text-slate-300 mb-1.5"
            >
              Paid by *
            </label>
            <select
              id="select-expense-payer"
              value={paidBy}
              onChange={(e) => setPaidBy(e.target.value)}
              className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 focus:outline-none focus:border-indigo-500 transition cursor-pointer"
            >
              {group.participants.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="block text-xs font-semibold text-slate-300">
              Split between ({splitAmong.length}/{group.participants.length})
            </span>
            <button
              type="button"
              onClick={handleToggleAll}
              className="text-xs text-indigo-400 hover:text-indigo-300 font-semibold transition cursor-pointer"
            >
              {splitAmong.length === group.participants.length ? "Deselect All" : "Select All"}
            </button>
          </div>

          <div className="space-y-1.5 max-h-44 overflow-y-auto pr-1">
            {group.participants.map((p) => {
              const checked = splitAmong.includes(p.id);
              return (
                <label
                  key={p.id}
                  className="flex items-center gap-3 p-2 rounded-xl border border-slate-800 bg-slate-950/60 hover:bg-slate-950 transition cursor-pointer"
                >
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={() => handleToggleParticipant(p.id)}
                    className="rounded bg-slate-900 border-slate-700 text-indigo-600 focus:ring-indigo-500 w-4 h-4 cursor-pointer"
                  />
                  <span className="text-xs font-medium text-slate-200">{p.name}</span>
                </label>
              );
            })}
          </div>

          {amountCents > 0 && splitAmong.length > 0 && (
            <p className="text-[11px] text-slate-400 mt-2 text-right">
              Approx.{" "}
              <strong className="text-indigo-300 font-mono">
                {formatMoney(perPersonCents, group.currency)}
              </strong>{" "}
              per selected person
            </p>
          )}
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
            {submitting ? "Saving..." : "Save Expense"}
          </button>
        </div>
      </form>
    </Modal>
  );
};
