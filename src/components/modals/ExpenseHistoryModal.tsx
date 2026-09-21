import type React from "react";
import type { Expense, Participant } from "../../types";
import { formatDateTime, formatMoney } from "../../utils/formatters";
import { Modal } from "../common/Modal";

interface ExpenseHistoryModalProps {
  isOpen: boolean;
  onClose: () => void;
  expense: Expense | null;
  currency: string;
  participants: Participant[];
}

export const ExpenseHistoryModal: React.FC<ExpenseHistoryModalProps> = ({
  isOpen,
  onClose,
  expense,
  currency,
  participants,
}) => {
  if (!expense) return null;

  const getParticipantName = (id: string) => {
    return participants.find((p) => p.id === id)?.name || "Unknown";
  };

  const historyEntries = expense.history ? [...expense.history].reverse() : [];

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Expense Revision History"
      icon="📜"
      maxWidthClass="max-w-lg"
    >
      <div className="space-y-4">
        {/* Current State Card */}
        <div className="p-3.5 rounded-xl bg-slate-900 border border-indigo-500/30">
          <div className="flex items-center justify-between mb-2">
            <span className="text-[11px] font-bold uppercase tracking-wider text-indigo-400">
              Current Version
            </span>
            {expense.updated_at ? (
              <span className="text-[11px] text-slate-400">
                Last modified {formatDateTime(expense.updated_at)}
              </span>
            ) : (
              <span className="text-[11px] text-slate-400">
                Created {formatDateTime(expense.created_at)}
              </span>
            )}
          </div>
          <div className="flex items-start justify-between gap-2">
            <div>
              <h4 className="text-sm font-semibold text-slate-100">{expense.title}</h4>
              <p className="text-xs text-slate-400 mt-0.5">
                Paid by{" "}
                <strong className="text-slate-200">{getParticipantName(expense.paid_by)}</strong>
              </p>
            </div>
            <div className="text-right">
              <span className="text-base font-bold font-mono text-indigo-300">
                {formatMoney(expense.amount_cents, currency)}
              </span>
            </div>
          </div>
          <div className="mt-2.5 pt-2 border-t border-slate-800 flex flex-wrap gap-1.5">
            {expense.splits.map((s) => (
              <span
                key={s.participant_id}
                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-slate-950 text-[11px] text-slate-300 border border-slate-800"
              >
                <span>{getParticipantName(s.participant_id)}</span>
                <span className="font-semibold text-indigo-400">
                  ({s.shares} {s.shares === 1 ? "part" : "parts"})
                </span>
              </span>
            ))}
          </div>
        </div>

        {/* Audit Timeline */}
        <div>
          <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-3">
            Change History ({historyEntries.length} {historyEntries.length === 1 ? "edit" : "edits"}
            )
          </h4>

          {historyEntries.length === 0 ? (
            <div className="text-center py-6 px-4 rounded-xl border border-dashed border-slate-800 bg-slate-950/40">
              <p className="text-xs text-slate-500">
                This expense has not been modified since creation.
              </p>
            </div>
          ) : (
            <div className="space-y-4 relative before:absolute before:inset-0 before:left-3 before:w-0.5 before:bg-slate-800 pl-7">
              {historyEntries.map((entry, idx) => (
                <div key={`${entry.edited_at}-${idx}`} className="relative">
                  {/* Timeline dot */}
                  <div className="absolute -left-7 top-1.5 w-2.5 h-2.5 rounded-full bg-indigo-500 border-2 border-slate-950 ring-2 ring-indigo-500/20" />

                  <div className="p-3 rounded-xl bg-slate-950/70 border border-slate-800">
                    <div className="flex items-center justify-between mb-1.5">
                      <span className="text-[11px] font-semibold text-indigo-300">
                        {formatDateTime(entry.edited_at)}
                      </span>
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-900 text-slate-400 border border-slate-800">
                        Revision #{historyEntries.length - idx}
                      </span>
                    </div>

                    <p className="text-xs font-medium text-amber-300/90 mb-2">{entry.summary}</p>

                    <div className="text-[11px] text-slate-400 space-y-1 bg-slate-900/60 p-2 rounded-lg border border-slate-850">
                      <div className="font-semibold text-slate-300 mb-1">
                        State before this edit:
                      </div>
                      <div className="flex justify-between">
                        <span>Title:</span>
                        <span className="text-slate-200">{entry.previous_title}</span>
                      </div>
                      <div className="flex justify-between">
                        <span>Amount:</span>
                        <span className="font-mono text-slate-200">
                          {formatMoney(entry.previous_amount_cents, currency)}
                        </span>
                      </div>
                      <div className="flex justify-between">
                        <span>Payer:</span>
                        <span className="text-slate-200">
                          {getParticipantName(entry.previous_paid_by)}
                        </span>
                      </div>
                      <div className="pt-1 mt-1 border-t border-slate-800">
                        <span className="block mb-1">Splits:</span>
                        <div className="flex flex-wrap gap-1">
                          {entry.previous_splits.map((s) => (
                            <span
                              key={s.participant_id}
                              className="px-1.5 py-0.5 rounded bg-slate-950 text-[10px] text-slate-300"
                            >
                              {getParticipantName(s.participant_id)} ({s.shares}p)
                            </span>
                          ))}
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="flex justify-end pt-2">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 rounded-xl text-xs font-semibold bg-slate-800 hover:bg-slate-700 text-slate-200 transition cursor-pointer"
          >
            Close
          </button>
        </div>
      </div>
    </Modal>
  );
};
