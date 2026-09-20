import type React from "react";
import type { Group, SettlementTransfer } from "../../types";
import { formatMoney } from "../../utils/formatters";

interface SettleUpTabProps {
  group: Group;
  settlements: SettlementTransfer[];
  onOpenReimburse: () => void;
  onMarkAsPaid: (fromId: string, toId: string, amount: string) => void;
}

export const SettleUpTab: React.FC<SettleUpTabProps> = ({
  group,
  settlements,
  onOpenReimburse,
  onMarkAsPaid,
}) => {
  return (
    <div className="space-y-4">
      {/* Information & Action Card */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-4 rounded-xl bg-indigo-950/30 border border-indigo-500/20 text-xs text-indigo-300">
        <div className="flex items-start gap-3">
          <div className="text-indigo-400 mt-0.5">
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
                d="m11.25 11.25.041-.02a.75.75 0 0 1 1.063.852l-.708 2.836a.75.75 0 0 0 1.063.853l.041-.021M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9-3.75h.008v.008H12V8.25Z"
              />
            </svg>
          </div>
          <div>
            <p className="font-medium text-indigo-200">Optimal Settlement Plan</p>
            <p className="text-indigo-300/80 mt-0.5">
              ezcount's greedy settlement algorithm minimizes the total number of transactions
              required to settle all group debts.
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={onOpenReimburse}
          className="self-start sm:self-auto inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white shadow-md shadow-emerald-600/20 transition active:scale-95 cursor-pointer shrink-0"
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
          <span>Record Reimbursement</span>
        </button>
      </div>

      {/* Settlements List */}
      {settlements.length === 0 ? (
        <div className="text-center py-12 px-4 border border-dashed border-slate-800 rounded-2xl bg-slate-900/30 space-y-2">
          <div className="w-10 h-10 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 mx-auto flex items-center justify-center text-lg">
            ✓
          </div>
          <h3 className="text-sm font-semibold text-slate-200">All settled up!</h3>
          <p className="text-xs text-slate-400">
            No one in this group owes anything to anyone. Everyone is squared up.
          </p>
        </div>
      ) : (
        <div className="space-y-2.5">
          {settlements.map((s) => (
            <div
              key={`${s.from_id}-${s.to_id}`}
              className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-4 rounded-xl border border-slate-800 bg-slate-900/50 hover:bg-slate-900/80 transition"
            >
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-rose-500/10 text-rose-400 border border-rose-500/20 flex items-center justify-center text-xs font-bold">
                  {s.from_name.charAt(0).toUpperCase()}
                </div>
                <div>
                  <p className="text-sm font-semibold text-white">
                    <span className="text-rose-300">{s.from_name}</span> pays{" "}
                    <span className="text-emerald-300">{s.to_name}</span>
                  </p>
                  <p className="text-[11px] text-slate-500">Direct reimbursement</p>
                </div>
              </div>

              <div className="flex items-center justify-between sm:justify-end gap-3 pt-2 sm:pt-0 border-t sm:border-t-0 border-slate-800/80">
                <span className="text-base font-bold text-emerald-400 font-mono">
                  {formatMoney(s.amount_cents, group.currency)}
                </span>
                <button
                  type="button"
                  onClick={() =>
                    onMarkAsPaid(s.from_id, s.to_id, (s.amount_cents / 100).toFixed(2))
                  }
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold bg-emerald-600/20 hover:bg-emerald-600 text-emerald-300 hover:text-white border border-emerald-500/30 transition cursor-pointer active:scale-95"
                >
                  <svg
                    className="w-3.5 h-3.5"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth="2.5"
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" d="m4.5 12.75 6 6 9-13.5" />
                  </svg>
                  <span>Mark as Paid</span>
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
