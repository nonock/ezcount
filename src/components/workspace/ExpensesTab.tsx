import type React from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Expense, Group } from "../../types";
import {
  formatDate,
  formatDateGroupHeader,
  formatMoney,
  getLocalDateKey,
} from "../../utils/formatters";

interface ExpensesTabProps {
  group: Group;
  hasOutstandingDebt?: boolean;
  onOpenAddExpense: () => void;
  onOpenReimburse: () => void;
  onDeleteExpense: (expenseId: string) => void;
  onEditExpense: (expense: Expense) => void;
  onViewHistory: (expense: Expense) => void;
}

interface DateGroup {
  dateKey: string;
  displayDate: string;
  totalCents: number;
  items: Expense[];
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
  // 1. Sort transactions descending (newest first)
  const sortedExpenses = [...group.expenses].sort((a, b) => {
    const timeA = new Date(a.created_at).getTime() || 0;
    const timeB = new Date(b.created_at).getTime() || 0;
    return timeB - timeA;
  });

  // 2. Infinite loader state (batch 10 by 10)
  const [visibleCount, setVisibleCount] = useState(10);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (group.id) {
      setVisibleCount(10);
    }
  }, [group.id]);

  const visibleExpenses = sortedExpenses.slice(0, visibleCount);

  const hasMore = visibleCount < sortedExpenses.length;
  const remainingCount = sortedExpenses.length - visibleCount;

  const handleLoadMore = () => {
    setVisibleCount((prev) => Math.min(prev + 10, sortedExpenses.length));
  };

  useEffect(() => {
    if (!hasMore) return;
    const sentinel = sentinelRef.current;
    if (!sentinel) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          setVisibleCount((prev) => Math.min(prev + 10, sortedExpenses.length));
        }
      },
      { rootMargin: "250px" }
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMore, sortedExpenses.length]);

  // 3. Group visible expenses by local calendar day
  const dateGroups = useMemo(() => {
    const map = new Map<string, DateGroup>();
    for (const exp of visibleExpenses) {
      const key = getLocalDateKey(exp.created_at);
      const existing = map.get(key);
      if (existing) {
        existing.items.push(exp);
        if (!exp.is_reimbursement) {
          existing.totalCents += exp.amount_cents;
        }
      } else {
        map.set(key, {
          dateKey: key,
          displayDate: formatDateGroupHeader(exp.created_at),
          totalCents: exp.is_reimbursement ? 0 : exp.amount_cents,
          items: [exp],
        });
      }
    }
    return Array.from(map.values());
  }, [visibleExpenses]);

  const totalCents = group.expenses.reduce((sum, e) => sum + e.amount_cents, 0);

  return (
    <div className="space-y-5">
      {/* Header & Action Toolbar */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 pb-1 border-b border-slate-800/60">
        <div>
          <div className="flex items-center gap-2.5">
            <h3 className="text-base font-bold text-white tracking-tight">Transaction History</h3>
            <span className="px-2 py-0.5 rounded-full text-xs font-semibold bg-slate-800 text-slate-300 tabular-nums">
              {group.expenses.length}
            </span>
          </div>
          <p className="text-xs text-slate-400 mt-0.5">
            Total recorded volume:{" "}
            <span className="font-semibold text-slate-200 font-mono tabular-nums">
              {formatMoney(totalCents, group.currency)}
            </span>
          </p>
        </div>

        <div className="flex items-center gap-2 sm:self-auto">
          {hasOutstandingDebt && (
            <button
              type="button"
              onClick={onOpenReimburse}
              className="inline-flex items-center justify-center gap-1.5 px-3.5 py-2 min-h-[44px] sm:min-h-0 rounded-xl text-xs font-semibold bg-emerald-950/50 hover:bg-emerald-900/60 text-emerald-300 border border-emerald-500/30 hover:border-emerald-500/50 transition-colors duration-150 active:scale-[0.98] cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-400"
            >
              <svg
                className="w-3.5 h-3.5 shrink-0"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth="2"
                aria-hidden="true"
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="m4.5 12.75 6 6 9-13.5" />
              </svg>
              <span>Reimburse</span>
            </button>
          )}

          <button
            type="button"
            onClick={onOpenAddExpense}
            className="inline-flex items-center justify-center gap-1.5 px-4 py-2 min-h-[44px] sm:min-h-0 rounded-xl text-xs font-semibold bg-indigo-600 hover:bg-indigo-500 text-white shadow-md shadow-indigo-600/20 transition-all duration-150 active:scale-[0.98] cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-950"
          >
            <svg
              className="w-3.5 h-3.5 shrink-0"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2.5"
              aria-hidden="true"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            <span>Add Expense</span>
          </button>
        </div>
      </div>

      {/* Expenses Ledger Stream */}
      {group.expenses.length === 0 ? (
        <div className="text-center py-16 px-6 border border-dashed border-slate-800 rounded-3xl bg-gradient-to-b from-slate-900/40 to-slate-950/60 space-y-3">
          <div
            aria-hidden="true"
            className="w-12 h-12 rounded-2xl bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 mx-auto flex items-center justify-center text-xl shadow-inner select-none"
          >
            🧾
          </div>
          <div className="space-y-1">
            <h4 className="text-sm font-bold text-white tracking-tight">
              No expenses recorded yet
            </h4>
            <p className="text-xs text-slate-400 max-w-sm mx-auto">
              Add your first shared expense or bill to start calculating fair balances.
            </p>
          </div>
          <button
            type="button"
            onClick={onOpenAddExpense}
            className="mt-3 inline-flex items-center gap-1.5 px-4 py-2 min-h-[44px] sm:min-h-0 text-xs font-semibold rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white transition-all duration-150 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
          >
            <svg
              className="w-3.5 h-3.5 shrink-0"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2.5"
              aria-hidden="true"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            <span>Add First Expense</span>
          </button>
        </div>
      ) : (
        <div className="space-y-6">
          {dateGroups.map((dg) => (
            <section
              key={dg.dateKey}
              aria-labelledby={`date-header-${dg.dateKey}`}
              className="space-y-2.5"
            >
              {/* Date Group Section Header */}
              <div
                id={`date-header-${dg.dateKey}`}
                className="flex items-center justify-between px-1 text-xs text-slate-400 font-medium"
              >
                <div className="flex items-center gap-2">
                  <span className="font-bold text-slate-200 tracking-tight text-xs sm:text-sm">
                    {dg.displayDate}
                  </span>
                  <span className="text-[11px] text-slate-500 font-normal">
                    • {dg.items.length} {dg.items.length === 1 ? "transaction" : "transactions"}
                  </span>
                </div>
                {dg.totalCents > 0 && (
                  <span className="text-slate-300 font-mono tabular-nums text-xs">
                    {formatMoney(dg.totalCents, group.currency)}
                  </span>
                )}
              </div>

              {/* Transactions for this date */}
              <ul className="space-y-2.5 list-none p-0 m-0">
                {dg.items.map((e) => {
                  const payerName = nameMap.get(e.paid_by) || "Unknown";
                  const isReimbursement = Boolean(e.is_reimbursement);
                  const hasHistory = Boolean(e.history && e.history.length > 0);

                  if (isReimbursement) {
                    const recipientId = e.splits?.[0]?.participant_id;
                    const recipientName = recipientId
                      ? nameMap.get(recipientId) || "Unknown"
                      : "Unknown";

                    return (
                      <li
                        key={e.id}
                        data-testid="expense-item"
                        className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-4 sm:p-4.5 rounded-2xl border border-emerald-500/25 bg-gradient-to-r from-emerald-950/20 via-slate-900/60 to-slate-900/60 hover:border-emerald-500/40 transition-colors duration-150 group"
                      >
                        <div className="flex items-start gap-3.5 min-w-0">
                          <div
                            aria-hidden="true"
                            className="w-10 h-10 rounded-xl bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 font-bold text-base shrink-0 select-none shadow-sm"
                          >
                            🤝
                          </div>
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2 flex-wrap">
                              <h4 className="text-sm font-bold text-white tracking-tight truncate">
                                {e.title}
                              </h4>
                              <span className="px-2 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wider bg-emerald-500/15 text-emerald-300 border border-emerald-500/30">
                                Reimbursement
                              </span>
                              {hasHistory && (
                                <button
                                  type="button"
                                  onClick={() => onViewHistory(e)}
                                  aria-label={`Edited (${e.history?.length}): View revision history for ${e.title}`}
                                  className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-semibold bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 border border-amber-500/30 transition-colors duration-150 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400"
                                  title="View edit history"
                                >
                                  Edited ({e.history?.length})
                                </button>
                              )}
                            </div>

                            {/* Directional transfer flow */}
                            <p className="text-xs text-slate-300 mt-1 flex items-center gap-1.5 flex-wrap">
                              <span>Paid by</span>
                              <span className="font-semibold text-white px-1.5 py-0.5 rounded bg-slate-800/80">
                                {payerName}
                              </span>
                              <span aria-hidden="true" className="text-emerald-400 font-bold">
                                →
                              </span>
                              <span className="font-semibold text-white px-1.5 py-0.5 rounded bg-slate-800/80">
                                {recipientName}
                              </span>
                            </p>

                            <div className="text-[11px] text-slate-500 mt-1">
                              {formatDate(e.created_at)}
                            </div>
                          </div>
                        </div>

                        {/* Amount & Actions */}
                        <div className="flex items-center justify-between sm:justify-end gap-3 pt-2 sm:pt-0 border-t sm:border-t-0 border-slate-800/60 shrink-0">
                          <div className="text-left sm:text-right">
                            <span className="text-base sm:text-lg font-bold text-emerald-400 font-mono tabular-nums tracking-tight">
                              {formatMoney(e.amount_cents, group.currency)}
                            </span>
                          </div>

                          <div className="flex items-center gap-1">
                            <button
                              type="button"
                              onClick={() => onEditExpense(e)}
                              aria-label={`Edit reimbursement: ${e.title}`}
                              className="opacity-80 sm:opacity-0 sm:group-hover:opacity-100 sm:group-focus-within:opacity-100 p-2.5 sm:p-1.5 min-h-[44px] min-w-[44px] sm:min-h-0 sm:min-w-0 rounded-lg text-slate-400 hover:text-indigo-400 hover:bg-indigo-500/10 transition-all duration-150 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
                              title="Edit reimbursement"
                            >
                              <svg
                                className="w-4 h-4"
                                fill="none"
                                viewBox="0 0 24 24"
                                stroke="currentColor"
                                strokeWidth="2"
                                aria-hidden="true"
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
                              aria-label={`Delete reimbursement: ${e.title}`}
                              className="opacity-80 sm:opacity-0 sm:group-hover:opacity-100 sm:group-focus-within:opacity-100 p-2.5 sm:p-1.5 min-h-[44px] min-w-[44px] sm:min-h-0 sm:min-w-0 rounded-lg text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 transition-all duration-150 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-500"
                              title="Delete reimbursement"
                            >
                              <svg
                                className="w-4 h-4"
                                fill="none"
                                viewBox="0 0 24 24"
                                stroke="currentColor"
                                strokeWidth="2"
                                aria-hidden="true"
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
                      </li>
                    );
                  }

                  const totalShares = e.splits.reduce((sum, s) => sum + s.shares, 0);
                  const sharePerPerson = formatMoney(
                    Math.floor(e.amount_cents / (totalShares || 1)),
                    group.currency
                  );

                  return (
                    <li
                      key={e.id}
                      data-testid="expense-item"
                      className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-4 sm:p-4.5 rounded-2xl border border-slate-800/90 bg-slate-900/60 hover:bg-slate-900/95 hover:border-slate-700/80 transition-colors duration-150 group"
                    >
                      <div className="flex items-start gap-3.5 min-w-0">
                        <div
                          aria-hidden="true"
                          className="w-10 h-10 rounded-xl bg-slate-800/90 border border-slate-700/60 flex items-center justify-center text-indigo-400 font-bold text-sm shrink-0 select-none shadow-sm"
                        >
                          {e.title.charAt(0).toUpperCase()}
                        </div>

                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2 flex-wrap">
                            <h4 className="text-sm font-bold text-white tracking-tight truncate">
                              {e.title}
                            </h4>
                            {hasHistory && (
                              <button
                                type="button"
                                onClick={() => onViewHistory(e)}
                                aria-label={`Edited (${e.history?.length}): View revision history for ${e.title}`}
                                className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-semibold bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 border border-amber-500/30 transition-colors duration-150 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400"
                                title="View edit history"
                              >
                                Edited ({e.history?.length})
                              </button>
                            )}
                          </div>

                          {/* Payer attribution */}
                          <p className="text-xs text-slate-300 mt-1">
                            Paid by{" "}
                            <span className="font-semibold text-white px-1.5 py-0.5 rounded bg-slate-800/80">
                              {payerName}
                            </span>
                          </p>

                          {/* Beneficiaries split chips */}
                          <div className="flex items-center gap-1.5 mt-2 flex-wrap">
                            <span className="text-[10px] text-slate-400 uppercase font-bold tracking-wider mr-0.5">
                              Split for:
                            </span>
                            {e.splits.map((s) => {
                              const pName = nameMap.get(s.participant_id) || "Unknown";
                              return (
                                <span
                                  key={s.participant_id}
                                  className="inline-flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-md bg-slate-800/80 text-slate-300 border border-slate-700/50"
                                >
                                  <span>{pName}</span>
                                  {s.shares > 1 && (
                                    <span className="text-indigo-400 font-bold">
                                      ({s.shares} parts)
                                    </span>
                                  )}
                                </span>
                              );
                            })}
                          </div>

                          <div className="text-[11px] text-slate-500 mt-1.5">
                            {formatDate(e.created_at)}
                          </div>
                        </div>
                      </div>

                      {/* Amount column & Actions */}
                      <div className="flex items-center justify-between sm:justify-end gap-3 pt-2 sm:pt-0 border-t sm:border-t-0 border-slate-800/60 shrink-0">
                        <div className="text-left sm:text-right">
                          <span className="text-base sm:text-lg font-bold text-white font-mono tabular-nums tracking-tight">
                            {formatMoney(e.amount_cents, group.currency)}
                          </span>
                          <span className="text-[11px] text-slate-400 block font-mono">
                            {sharePerPerson} / part
                          </span>
                        </div>

                        <div className="flex items-center gap-1">
                          <button
                            type="button"
                            onClick={() => onEditExpense(e)}
                            aria-label={`Edit expense: ${e.title}`}
                            className="opacity-80 sm:opacity-0 sm:group-hover:opacity-100 sm:group-focus-within:opacity-100 p-2.5 sm:p-1.5 min-h-[44px] min-w-[44px] sm:min-h-0 sm:min-w-0 rounded-lg text-slate-400 hover:text-indigo-400 hover:bg-indigo-500/10 transition-all duration-150 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
                            title="Edit expense"
                          >
                            <svg
                              className="w-4 h-4"
                              fill="none"
                              viewBox="0 0 24 24"
                              stroke="currentColor"
                              strokeWidth="2"
                              aria-hidden="true"
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
                            aria-label={`Delete expense: ${e.title}`}
                            className="opacity-80 sm:opacity-0 sm:group-hover:opacity-100 sm:group-focus-within:opacity-100 p-2.5 sm:p-1.5 min-h-[44px] min-w-[44px] sm:min-h-0 sm:min-w-0 rounded-lg text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 transition-all duration-150 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-500"
                            title="Delete expense"
                          >
                            <svg
                              className="w-4 h-4"
                              fill="none"
                              viewBox="0 0 24 24"
                              stroke="currentColor"
                              strokeWidth="2"
                              aria-hidden="true"
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
                    </li>
                  );
                })}
              </ul>
            </section>
          ))}

          {/* Infinite Scroll Sentinel & Load More Controls */}
          {hasMore && (
            <div ref={sentinelRef} className="pt-2 pb-2 text-center">
              <button
                type="button"
                onClick={handleLoadMore}
                className="inline-flex items-center gap-2 px-4 py-2 min-h-[44px] sm:min-h-0 text-xs font-semibold rounded-xl bg-slate-900 hover:bg-slate-800 text-slate-300 border border-slate-800 hover:border-slate-700 transition cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
              >
                <span>Load 10 more transactions</span>
                <span className="text-slate-500 font-normal">({remainingCount} remaining)</span>
              </button>
            </div>
          )}

          {/* Showing counter */}
          {sortedExpenses.length > 10 && (
            <div className="text-center text-[11px] text-slate-500 pt-1">
              Showing {visibleExpenses.length} of {sortedExpenses.length} transactions
            </div>
          )}
        </div>
      )}
    </div>
  );
};
