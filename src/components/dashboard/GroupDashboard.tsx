import type React from "react";
import type { Group } from "../../types";
import { formatMoney } from "../../utils/formatters";

interface GroupDashboardProps {
  groups: Group[];
  onSelectGroup: (groupId: string) => void;
  onOpenCreateGroup: () => void;
}

export const GroupDashboard: React.FC<GroupDashboardProps> = ({
  groups,
  onSelectGroup,
  onOpenCreateGroup,
}) => {
  return (
    <div className="space-y-8 animate-in fade-in duration-200">
      {/* Hero Banner */}
      <div className="relative overflow-hidden rounded-3xl border border-slate-800 bg-gradient-to-br from-slate-900 via-slate-900/90 to-indigo-950/40 p-6 sm:p-8">
        <div className="absolute -right-12 -top-12 w-64 h-64 bg-indigo-500/10 rounded-full blur-3xl pointer-events-none" />
        <div className="relative z-10 max-w-2xl space-y-3">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-semibold bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
            Zero-Loss Debt Simplification
          </div>
          <h1 className="text-2xl sm:text-3xl font-extrabold text-white tracking-tight">
            Share expenses effortlessly. Settle debts without friction.
          </h1>
          <p className="text-sm text-slate-400 leading-relaxed">
            ezcount groups group expenses, divides bills with exact-cent precision, and uses a
            greedy settlement algorithm to compute the minimum possible repayments.
          </p>
        </div>
      </div>

      {/* Groups Section Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-bold text-white tracking-tight">Your Groups</h2>
          <p className="text-xs text-slate-400">
            {groups.length} active {groups.length === 1 ? "group" : "groups"}
          </p>
        </div>
        {groups.length > 0 && (
          <button
            type="button"
            onClick={onOpenCreateGroup}
            className="text-xs font-semibold text-indigo-400 hover:text-indigo-300 transition cursor-pointer"
          >
            + Create another
          </button>
        )}
      </div>

      {/* Groups Grid / Empty State */}
      {groups.length === 0 ? (
        <div className="text-center py-16 px-4 border border-dashed border-slate-800 rounded-2xl bg-slate-900/40 space-y-3">
          <div className="w-12 h-12 rounded-2xl bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 mx-auto flex items-center justify-center text-xl">
            🌴
          </div>
          <h3 className="text-base font-bold text-white">No groups yet</h3>
          <p className="text-xs text-slate-400 max-w-sm mx-auto">
            Create a group for your next trip, dinner, flatshare, or event to start splitting bills.
          </p>
          <button
            type="button"
            onClick={onOpenCreateGroup}
            className="mt-2 inline-flex items-center gap-1.5 px-4 py-2 rounded-xl text-xs font-semibold bg-indigo-600 hover:bg-indigo-500 text-white shadow-lg shadow-indigo-600/20 transition cursor-pointer"
          >
            + Create Group
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {groups.map((group) => {
            const totalCents = group.expenses.reduce((sum, e) => sum + e.amount_cents, 0);
            return (
              <button
                type="button"
                key={group.id}
                onClick={() => onSelectGroup(group.id)}
                className="text-left p-5 rounded-2xl border border-slate-800 bg-slate-900/60 hover:bg-slate-900 hover:border-slate-700 transition duration-200 group cursor-pointer space-y-4 relative overflow-hidden"
              >
                <div className="flex items-start justify-between">
                  <div className="space-y-1">
                    <span className="px-2 py-0.5 rounded text-[10px] font-bold bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">
                      {group.currency}
                    </span>
                    <h3 className="text-base font-bold text-white group-hover:text-indigo-200 transition">
                      {group.name}
                    </h3>
                  </div>
                  <div className="w-8 h-8 rounded-lg bg-slate-800 flex items-center justify-center text-slate-400 group-hover:text-white transition">
                    →
                  </div>
                </div>

                <div className="pt-2 border-t border-slate-800/80 flex items-center justify-between text-xs">
                  <div>
                    <span className="text-slate-500 block text-[10px] uppercase font-semibold">
                      Total Spent
                    </span>
                    <span className="text-sm font-bold text-slate-200 font-mono">
                      {formatMoney(totalCents, group.currency)}
                    </span>
                  </div>
                  <div className="text-right">
                    <span className="text-slate-500 block text-[10px] uppercase font-semibold">
                      Members
                    </span>
                    <span className="text-xs font-medium text-slate-300">
                      {group.participants.length} people • {group.expenses.length} records
                    </span>
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
};
