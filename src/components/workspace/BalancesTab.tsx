import type React from "react";
import { useMemo } from "react";
import type { Group, ParticipantBalance } from "../../types";
import { formatMoney } from "../../utils/formatters";

interface BalancesTabProps {
  group: Group;
  balances: ParticipantBalance[];
  onReimburseParticipant: (participantId: string, amount: string) => void;
}

export const BalancesTab: React.FC<BalancesTabProps> = ({
  group,
  balances,
  onReimburseParticipant,
}) => {
  const maxAbs = useMemo(
    () => Math.max(...balances.map((b) => Math.abs(b.net_cents)), 1),
    [balances]
  );

  return (
    <div className="space-y-4">
      <div className="p-4 rounded-xl bg-slate-900/50 border border-slate-800/80 text-xs text-slate-400">
        Positive amounts in <span className="text-emerald-400 font-semibold">green</span> mean the
        participant is owed money back. Negative amounts in{" "}
        <span className="text-rose-400 font-semibold">red</span> mean they need to pay.
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {balances.map((b) => {
          const isPositive = b.net_cents > 0;
          const isNegative = b.net_cents < 0;
          const isEven = b.net_cents === 0;

          const percentage = Math.min(100, Math.round((Math.abs(b.net_cents) / maxAbs) * 100));

          const badgeColor = isPositive
            ? "text-emerald-400 bg-emerald-500/10 border-emerald-500/20"
            : isNegative
              ? "text-rose-400 bg-rose-500/10 border-rose-500/20"
              : "text-slate-400 bg-slate-800 border-slate-700";

          const barColor = isPositive
            ? "bg-emerald-500"
            : isNegative
              ? "bg-rose-500"
              : "bg-slate-700";
          const statusText = isPositive ? "Gets back" : isNegative ? "Owes" : "Settled up";

          return (
            <div
              key={b.participant_id}
              className="p-4 rounded-xl border border-slate-800 bg-slate-900/50 space-y-3"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2.5">
                  <div className="w-8 h-8 rounded-full bg-slate-800 flex items-center justify-center text-xs font-bold text-slate-200">
                    {b.participant_name.charAt(0).toUpperCase()}
                  </div>
                  <div>
                    <h4 className="text-sm font-semibold text-white">
                      {b.participant_name}
                      {b.removed && (
                        <span className="ml-1.5 px-1.5 py-0.5 rounded text-[10px] font-medium bg-slate-800 text-slate-400 align-middle">
                          Removed
                        </span>
                      )}
                    </h4>
                    <span className="text-[11px] text-slate-400">{statusText}</span>
                  </div>
                </div>
                <div
                  className={`px-2.5 py-1 rounded-lg border text-xs font-bold font-mono tabular-nums ${badgeColor}`}
                >
                  {formatMoney(b.net_cents, group.currency)}
                </div>
              </div>

              {/* Progress bar */}
              <div className="w-full bg-slate-950 rounded-full h-2 overflow-hidden">
                <div
                  className={`${barColor} h-2 rounded-full transition-all duration-300`}
                  style={{ width: `${isEven ? 0 : percentage}%` }}
                />
              </div>

              <div className="flex items-center justify-between text-[11px] text-slate-400 pt-1 border-t border-slate-800/60">
                <span>
                  Paid:{" "}
                  <strong className="text-slate-200 font-mono tabular-nums">
                    {formatMoney(b.paid_cents, group.currency)}
                  </strong>
                </span>
                <span>
                  Consumed:{" "}
                  <strong className="text-slate-200 font-mono tabular-nums">
                    {formatMoney(b.owed_cents, group.currency)}
                  </strong>
                </span>
              </div>

              {isNegative && (
                <div className="pt-1">
                  <button
                    type="button"
                    onClick={() =>
                      onReimburseParticipant(
                        b.participant_id,
                        (Math.abs(b.net_cents) / 100).toFixed(2)
                      )
                    }
                    className="w-full py-2 min-h-[44px] sm:min-h-0 rounded-xl text-xs font-semibold bg-slate-800/80 hover:bg-emerald-950/50 text-slate-300 hover:text-emerald-300 border border-slate-700 hover:border-emerald-500/40 transition-colors duration-150 flex items-center justify-center gap-1.5 cursor-pointer touch-manipulation active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-400"
                  >
                    <span>Reimburse debt</span>
                    <span className="text-[10px] text-slate-400 font-mono tabular-nums font-normal">
                      ({formatMoney(Math.abs(b.net_cents), group.currency)})
                    </span>
                  </button>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};
