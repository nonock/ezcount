import type React from "react";
import type { Expense, Group } from "../../types";
import { formatDate, formatMoney } from "../../utils/formatters";

interface ExpensesTabProps {
  group: Group;
  hasOutstandingDebt?: boolean;
  onOpenAddExpense: () => void;
  onOpenReimburse: () => void;
  onDeleteExpense: (expenseId: string) => void;
  onEditExpense: (expense: Expense) => void;
  onViewHistory: (expense: Expense) => void;
}

export const ExpensesTab: React.FC<ExpensesTabProps> = ({
  group,
  hasOutstandingDebt = false,
  onOpenAddExpense,
  onOpenReimburse,
  onDeleteExpense,
  onEditExpense,
  onViewHistory,
}) => {
  const nameMap = new Map(group.participants.map((p) => [p.id, p.name]));

  return (
    <div className="space-y-4">
      {/* Top action bar */}
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-slate-300">Transaction History</h3>
        <div className="flex items-center gap-2">
          {hasOutstandingDebt && (
            <button
              type="button"
              onClick={onOpenReimburse}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-medium bg-emerald-950/40 hover:bg-emerald-900/60 text-emerald-300 border border-emerald-500/30 hover:border-emerald-500/50 transition active:scale-95 cursor-pointer"
            >
              <svg
                className="w-3.5 h-3.5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="m4.5 12.75 6 6 9-13.5" />
              </svg>
              <span>Reimburse</span>
            </button>
          )}
          <button
            type="button"
            onClick={onOpenAddExpense}
            className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl text-xs font-semibold bg-indigo-600 hover:bg-indigo-500 text-white shadow-md shadow-indigo-600/20 transition active:scale-95 cursor-pointer"
          >
            <svg
              className="w-3.5 h-3.5"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2.5"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            <span>Add Expense</span>
          </button>
        </div>
      </div>

      {/* Expenses List */}
      {group.expenses.length === 0 ? (
        <div className="text-center py-14 px-4 border border-dashed border-slate-800 rounded-2xl bg-slate-900/30 space-y-2">
          <h3 className="text-sm font-semibold text-slate-200">No expenses recorded yet</h3>
          <p className="text-xs text-slate-400 max-w-xs mx-auto">
            Click "Add Expense" to log the first bill for this group.
          </p>
          <button
            type="button"
            onClick={onOpenAddExpense}
            className="mt-2 px-4 py-2 text-xs font-semibold rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white transition cursor-pointer"
          >
            + Add Expense
          </button>
        </div>
      ) : (
        <div className="space-y-2.5">
          {group.expenses.map((e) => {
            const payerName = nameMap.get(e.paid_by) || "Unknown";
            const isReimbursement = !!e.is_reimbursement;
            const hasHistory = Boolean(e.history && e.history.length > 0);

            if (isReimbursement) {
              const recipientId = e.splits?.[0]?.participant_id;
              const recipientName = recipientId ? nameMap.get(recipientId) || "Unknown" : "Unknown";
              return (
                <div
                  key={e.id}
                  className="flex items-center justify-between p-4 rounded-xl border border-emerald-500/20 bg-emerald-950/10 hover:bg-emerald-950/20 transition group"
                >
                  <div className="flex items-center gap-3.5">
                    <div className="w-10 h-10 rounded-xl bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 font-bold text-base">
                      🤝
                    </div>
                    <div>
                      <div className="flex items-center gap-2 flex-wrap">
                        <h4 className="text-sm font-semibold text-white">{e.title}</h4>
                        <span className="px-2 py-0.5 rounded text-[10px] font-bold bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
                          Reimbursement
                        </span>
                        {hasHistory && (
                          <button
                            type="button"
                            onClick={() => onViewHistory(e)}
                            className="px-1.5 py-0.5 rounded text-[10px] font-semibold bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 border border-amber-500/30 transition cursor-pointer"
                            title="View edit history"
                          >
                            Edited ({e.history?.length})
                          </button>
                        )}
                      </div>
                      <p className="text-xs text-slate-400 mt-0.5">
                        <span className="font-medium text-slate-200">{payerName}</span> paid{" "}
                        <span className="font-medium text-slate-200">{recipientName}</span> directly
                      </p>
                      <p className="text-[11px] text-slate-500 mt-0.5">
                        {formatDate(e.created_at)}
                      </p>
                    </div>
                  </div>

                  <div className="flex items-center gap-2">
                    <div className="text-right mr-1">
                      <span className="text-sm font-bold text-emerald-400 font-mono">
                        {formatMoney(e.amount_cents, group.currency)}
                      </span>
                    </div>
                    <button
                      type="button"
                      onClick={() => onEditExpense(e)}
                      className="opacity-40 group-hover:opacity-100 p-1.5 rounded-lg text-slate-400 hover:text-indigo-400 hover:bg-indigo-500/10 transition cursor-pointer"
                      title="Edit reimbursement"
                    >
                      <svg
                        className="w-4 h-4"
                        fill="none"
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                        strokeWidth="2"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0 1 15.75 21H5.25A2.25 2.25 0 0 1 3 18.75V8.25A2.25 2.25 0 0 1 5.25 6H10"
                        />
                      </svg>
                    </button>
                    <button
                      type="button"
                      onClick={() => onDeleteExpense(e.id)}
                      className="opacity-40 group-hover:opacity-100 p-1.5 rounded-lg text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 transition cursor-pointer"
                      title="Delete reimbursement"
                    >
                      <svg
                        className="w-4 h-4"
                        fill="none"
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                        strokeWidth="2"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"
                        />
                      </svg>
                    </button>
                  </div>
                </div>
              );
            }

            const totalShares = e.splits.reduce((sum, s) => sum + s.shares, 0);
            const hasWeightedShares = e.splits.some((s) => s.shares > 1);
            const splitSummary = e.splits
              .map((s) => {
                const name = nameMap.get(s.participant_id) || "Unknown";
                return s.shares > 1 ? `${name} (${s.shares} parts)` : name;
              })
              .join(", ");

            const sharePerPerson = formatMoney(
              Math.floor(e.amount_cents / (totalShares || 1)),
              group.currency
            );

            return (
              <div
                key={e.id}
                className="flex items-center justify-between p-4 rounded-xl border border-slate-800 bg-slate-900/50 hover:bg-slate-900/90 transition group"
              >
                <div className="flex items-center gap-3.5">
                  <div className="w-10 h-10 rounded-xl bg-slate-800/80 border border-slate-700/50 flex items-center justify-center text-indigo-400 font-bold text-sm">
                    {e.title.charAt(0).toUpperCase()}
                  </div>
                  <div>
                    <div className="flex items-center gap-2 flex-wrap">
                      <h4 className="text-sm font-semibold text-white">{e.title}</h4>
                      {hasHistory && (
                        <button
                          type="button"
                          onClick={() => onViewHistory(e)}
                          className="px-1.5 py-0.5 rounded text-[10px] font-semibold bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 border border-amber-500/30 transition cursor-pointer"
                          title="View edit history"
                        >
                          Edited ({e.history?.length})
                        </button>
                      )}
                    </div>
                    <p className="text-xs text-slate-400 mt-0.5">
                      Paid by <span className="font-medium text-slate-300">{payerName}</span> •{" "}
                      {hasWeightedShares
                        ? `for ${e.splits.length} members (${totalShares} parts, ${sharePerPerson}/part)`
                        : `for ${e.splits.length} members (${sharePerPerson} each)`}
                    </p>
                    <p className="text-[11px] text-slate-500 mt-0.5">
                      {formatDate(e.created_at)} • [{splitSummary}]
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  <div className="text-right mr-1">
                    <span className="text-sm font-bold text-white font-mono">
                      {formatMoney(e.amount_cents, group.currency)}
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={() => onEditExpense(e)}
                    className="opacity-40 group-hover:opacity-100 p-1.5 rounded-lg text-slate-400 hover:text-indigo-400 hover:bg-indigo-500/10 transition cursor-pointer"
                    title="Edit expense"
                  >
                    <svg
                      className="w-4 h-4"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      strokeWidth="2"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0 1 15.75 21H5.25A2.25 2.25 0 0 1 3 18.75V8.25A2.25 2.25 0 0 1 5.25 6H10"
                      />
                    </svg>
                  </button>
                  <button
                    type="button"
                    onClick={() => onDeleteExpense(e.id)}
                    className="opacity-40 group-hover:opacity-100 p-1.5 rounded-lg text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 transition cursor-pointer"
                    title="Delete expense"
                  >
                    <svg
                      className="w-4 h-4"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      strokeWidth="2"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"
                      />
                    </svg>
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
