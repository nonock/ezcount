import type React from "react";
import { useEffect, useState } from "react";
import type { Expense, ExpenseSplit, Group } from "../../types";
import { formatMoney } from "../../utils/formatters";
import { Modal } from "../common/Modal";

import { formatDateInput } from "../../utils/formatters";

interface AddExpenseModalProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group;
  onAddExpense: (
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[],
    createdAt?: string | null
  ) => Promise<void>;
  editingExpense?: Expense | null;
  onUpdateExpense?: (
    expenseId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[],
    createdAt?: string | null
  ) => Promise<void>;
  currentUserId?: string | null;
}

interface SplitItemState {
  included: boolean;
  shares: number;
}

export const AddExpenseModal: React.FC<AddExpenseModalProps> = ({
  isOpen,
  onClose,
  group,
  onAddExpense,
  editingExpense,
  onUpdateExpense,
  currentUserId,
}) => {
  const [title, setTitle] = useState("");
  const [amountStr, setAmountStr] = useState("");
  const [paidBy, setPaidBy] = useState("");
  const [expenseDate, setExpenseDate] = useState<string>(formatDateInput());
  const [splitsState, setSplitsState] = useState<Record<string, SplitItemState>>({});
  const [submitting, setSubmitting] = useState(false);

  // Initialize or reset form state when modal opens or editingExpense changes
  useEffect(() => {
    if (!isOpen) return;

    if (editingExpense) {
      setTitle(editingExpense.title);
      setAmountStr((editingExpense.amount_cents / 100).toFixed(2));
      setPaidBy(editingExpense.paid_by);
      setExpenseDate(formatDateInput(editingExpense.created_at));

      const initialSplits: Record<string, SplitItemState> = {};
      for (const p of group.participants) {
        const match = editingExpense.splits.find((s) => s.participant_id === p.id);
        initialSplits[p.id] = {
          included: !!match,
          shares: match ? match.shares : 1,
        };
      }
      setSplitsState(initialSplits);
    } else {
      setTitle("");
      setAmountStr("");
      setExpenseDate(formatDateInput());
      const defaultPayer =
        currentUserId && group.participants.some((p) => p.id === currentUserId)
          ? currentUserId
          : group.participants[0]?.id || "";
      setPaidBy(defaultPayer);
      const initialSplits: Record<string, SplitItemState> = {};
      for (const p of group.participants) {
        initialSplits[p.id] = { included: true, shares: 1 };
      }
      setSplitsState(initialSplits);
    }
  }, [isOpen, editingExpense, group, currentUserId]);

  const handleToggleParticipant = (id: string) => {
    setSplitsState((prev) => ({
      ...prev,
      [id]: {
        included: !prev[id]?.included,
        shares: prev[id]?.shares || 1,
      },
    }));
  };

  const handleUpdateShares = (id: string, newShares: number) => {
    if (newShares < 1) return;
    setSplitsState((prev) => ({
      ...prev,
      [id]: {
        ...prev[id],
        shares: newShares,
      },
    }));
  };

  const includedParticipants = group.participants.filter((p) => splitsState[p.id]?.included);
  const totalShares = includedParticipants.reduce(
    (sum, p) => sum + (splitsState[p.id]?.shares || 1),
    0
  );

  const handleToggleAll = () => {
    const allIncluded = includedParticipants.length === group.participants.length;
    setSplitsState((prev) => {
      const next: Record<string, SplitItemState> = {};
      for (const p of group.participants) {
        next[p.id] = {
          included: !allIncluded,
          shares: prev[p.id]?.shares || 1,
        };
      }
      return next;
    });
  };

  const amountDecimal = Number.parseFloat(amountStr);
  const amountCents = !Number.isNaN(amountDecimal) ? Math.round(amountDecimal * 100) : 0;
  const perShareCents = totalShares > 0 ? Math.floor(amountCents / totalShares) : 0;

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
    if (includedParticipants.length === 0) {
      alert("Please select at least one participant to split the bill with");
      return;
    }

    const splits: ExpenseSplit[] = includedParticipants.map((p) => ({
      participant_id: p.id,
      shares: splitsState[p.id]?.shares || 1,
    }));

    let createdAtIso: string | null = null;
    if (expenseDate) {
      const [year, month, day] = expenseDate.split("-").map(Number);
      const d = new Date();
      d.setFullYear(year, month - 1, day);
      createdAtIso = d.toISOString();
    }

    setSubmitting(true);
    try {
      if (editingExpense && onUpdateExpense) {
        await onUpdateExpense(
          editingExpense.id,
          trimmedTitle,
          amountCents,
          paidBy,
          splits,
          createdAtIso
        );
      } else {
        await onAddExpense(trimmedTitle, amountCents, paidBy, splits, createdAtIso);
      }
      setTitle("");
      setAmountStr("");
      setExpenseDate(formatDateInput());
      onClose();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to save expense");
    } finally {
      setSubmitting(false);
    }
  };

  const isEditing = Boolean(editingExpense);

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={isEditing ? "Edit Expense" : "Add New Expense"}
      icon={isEditing ? "✏️" : "🧾"}
      maxWidthClass="max-w-lg"
    >
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

        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
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
                className="w-full pl-3.5 pr-12 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm font-semibold text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition font-mono tabular-nums"
              />
              <div className="absolute right-3 top-1/2 -translate-y-1/2 text-xs font-bold text-slate-400 pointer-events-none">
                {group.currency}
              </div>
            </div>
          </div>

          <div>
            <label
              htmlFor="input-expense-date"
              className="block text-xs font-semibold text-slate-300 mb-1.5"
            >
              Date *
            </label>
            <input
              id="input-expense-date"
              type="date"
              required
              value={expenseDate}
              onChange={(e) => setExpenseDate(e.target.value)}
              className="w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition [color-scheme:dark]"
            />
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
                  {p.id === currentUserId ? " (You)" : ""}
                </option>
              ))}
            </select>
          </div>
        </div>

        {/* Split Section with Weighted Parts */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="block text-xs font-semibold text-slate-300">
              Split between ({includedParticipants.length}/{group.participants.length} selected,{" "}
              {totalShares} parts)
            </span>
            <button
              type="button"
              onClick={handleToggleAll}
              className="text-xs text-indigo-400 hover:text-indigo-300 font-semibold transition cursor-pointer"
            >
              {includedParticipants.length === group.participants.length
                ? "Deselect All"
                : "Select All"}
            </button>
          </div>

          <div className="space-y-1.5 max-h-52 overflow-y-auto pr-1">
            {group.participants.map((p) => {
              const state = splitsState[p.id] || { included: false, shares: 1 };
              const isIncluded = state.included;
              const userShares = state.shares;
              const userOwed =
                totalShares > 0 && amountCents > 0
                  ? Math.round((amountCents * userShares) / totalShares)
                  : 0;

              return (
                <div
                  key={p.id}
                  className={`flex items-center justify-between p-2.5 rounded-xl border transition ${
                    isIncluded
                      ? "border-slate-800 bg-slate-900/80"
                      : "border-slate-850 bg-slate-950/40 opacity-55"
                  }`}
                >
                  <label className="flex items-center gap-2.5 cursor-pointer select-none">
                    <input
                      type="checkbox"
                      checked={isIncluded}
                      onChange={() => handleToggleParticipant(p.id)}
                      className="rounded bg-slate-900 border-slate-700 text-indigo-600 focus:ring-indigo-500 w-4 h-4 cursor-pointer"
                    />
                    <span className="text-xs font-medium text-slate-200">{p.name}</span>
                  </label>

                  {isIncluded && (
                    <div className="flex items-center gap-2.5">
                      {amountCents > 0 && totalShares > 0 && (
                        <span className="text-xs font-mono font-semibold text-indigo-300">
                          {formatMoney(userOwed, group.currency)}
                        </span>
                      )}

                      {/* Stepper for parts */}
                      <div className="inline-flex items-center rounded-lg border border-slate-700 bg-slate-950 p-0.5">
                        <button
                          type="button"
                          onClick={() => handleUpdateShares(p.id, userShares - 1)}
                          disabled={userShares <= 1}
                          className="w-5 h-5 flex items-center justify-center text-xs font-bold text-slate-400 hover:text-white disabled:opacity-30 transition cursor-pointer disabled:cursor-not-allowed"
                          title="Decrease parts"
                        >
                          -
                        </button>
                        <span className="px-1.5 text-[11px] font-bold text-slate-200 min-w-14 text-center">
                          {userShares} {userShares === 1 ? "part" : "parts"}
                        </span>
                        <button
                          type="button"
                          onClick={() => handleUpdateShares(p.id, userShares + 1)}
                          className="w-5 h-5 flex items-center justify-center text-xs font-bold text-slate-400 hover:text-white transition cursor-pointer"
                          title="Increase parts"
                        >
                          +
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>

          {amountCents > 0 && totalShares > 0 && (
            <div className="flex items-center justify-between text-[11px] text-slate-400 mt-2 px-1">
              <span>
                Total: <strong className="text-slate-200">{totalShares} parts</strong>
              </span>
              <span>
                Approx.{" "}
                <strong className="text-indigo-300 font-mono">
                  {formatMoney(perShareCents, group.currency)}
                </strong>{" "}
                per part
              </span>
            </div>
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
            {submitting ? "Saving..." : isEditing ? "Save Changes" : "Save Expense"}
          </button>
        </div>
      </form>
    </Modal>
  );
};
