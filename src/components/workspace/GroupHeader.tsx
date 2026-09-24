import type React from "react";
import { useMemo } from "react";
import type { Group, ParticipantBalance, SyncInfo } from "../../types";
import { formatMoney } from "../../utils/formatters";

interface GroupHeaderProps {
  group: Group;
  balances: ParticipantBalance[];
  currentUserId: string | null;
  onSelectCurrentUser: (id: string) => void;
  onOpenAddMember: () => void;
  onRemoveMember: (participantId: string) => void;
  onOpenShare: () => void;
  onDeleteGroup: () => void;
  syncInfo: SyncInfo | null;
}

export const GroupHeader: React.FC<GroupHeaderProps> = ({
  group,
  balances,
  currentUserId,
  onSelectCurrentUser,
  onOpenAddMember,
  onRemoveMember,
  onOpenShare,
  onDeleteGroup,
  syncInfo,
}) => {
  const activeParticipants = useMemo(
    () => group.participants.filter((p) => !p.removed),
    [group.participants]
  );

  const totalCents = useMemo(
    () => group.expenses.reduce((sum, e) => sum + e.amount_cents, 0),
    [group.expenses]
  );

  const currentUserBalance = useMemo(
    () => balances.find((b) => b.participant_id === currentUserId),
    [balances, currentUserId]
  );

  const userPaidCents = useMemo(
    () =>
      group.expenses
        .filter((e) => e.paid_by === currentUserId && !e.is_reimbursement)
        .reduce((sum, e) => sum + e.amount_cents, 0),
    [group.expenses, currentUserId]
  );

  const userShareCents = currentUserBalance?.owed_cents || 0;
  const userNetCents = currentUserBalance?.net_cents || 0;

  return (
    <div className="bg-slate-900/60 border border-slate-800 rounded-2xl p-4 sm:p-5 backdrop-blur-md space-y-4">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <div className="flex items-center gap-3">
            <h2 className="text-xl font-bold text-white tracking-tight">{group.name}</h2>
            <span className="px-2 py-0.5 rounded-md text-xs font-semibold bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">
              {group.currency}
            </span>
          </div>
          <p className="text-xs text-slate-400 mt-1 flex items-center gap-2 flex-wrap">
            <span>{activeParticipants.length} participants</span>
            <span aria-hidden="true" className="text-slate-600">
              •
            </span>
            <span>
              Group Total:{" "}
              <strong className="text-slate-200 font-mono tabular-nums">
                {formatMoney(totalCents, group.currency)}
              </strong>
            </span>
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={onOpenShare}
            title={
              syncInfo?.last_error
                ? `Last sync failed: ${syncInfo.last_error}`
                : syncInfo?.enabled
                  ? "Synced with other members"
                  : "Share this group"
            }
            className="inline-flex items-center gap-1.5 px-3 py-1.5 min-h-[44px] sm:min-h-0 rounded-xl text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 hover:border-slate-600 transition-colors duration-150 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
          >
            {syncInfo?.enabled && (
              <span
                aria-hidden="true"
                className={`w-1.5 h-1.5 rounded-full ${
                  syncInfo.last_error ? "bg-rose-400" : "bg-emerald-400"
                }`}
              />
            )}
            <span>
              {syncInfo?.enabled ? (syncInfo.last_error ? "Sync issue" : "Shared") : "Share"}
            </span>
          </button>
          <button
            type="button"
            onClick={onOpenAddMember}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 min-h-[44px] sm:min-h-0 rounded-xl text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 hover:border-slate-600 transition-colors duration-150 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
          >
            <svg
              className="w-3.5 h-3.5"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2"
              aria-hidden="true"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M18 7.5v3m0 0v3m0-3h3m-3 0h-3m-2.25-4.125a3.375 3.375 0 1 1-6.75 0 3.375 3.375 0 0 1 6.75 0ZM3 19.235v-.11a6.375 6.375 0 0 1 12.75 0v.109A12.318 12.318 0 0 1 9.374 21c-2.331 0-4.512-.645-6.374-1.765Z"
              />
            </svg>
            <span>Add Member</span>
          </button>
          <button
            type="button"
            onClick={onDeleteGroup}
            aria-label={`Delete group: ${group.name}`}
            className="p-2 sm:p-1.5 min-h-[44px] min-w-[44px] sm:min-h-0 sm:min-w-0 rounded-xl text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 transition-colors duration-150 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-500"
            title="Delete group"
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

      {/* Member avatars / pills */}
      <div className="flex flex-wrap gap-1.5 pt-1 border-t border-slate-800/80">
        {activeParticipants.map((p) => (
          <span
            key={p.id}
            className="inline-flex items-center gap-1.5 pl-2.5 pr-1 py-1 rounded-lg bg-slate-800/70 border border-slate-700/60 text-xs font-medium text-slate-300"
          >
            <span className="w-4 h-4 rounded-full bg-indigo-500/20 text-indigo-300 text-[10px] font-bold flex items-center justify-center">
              {p.name.charAt(0).toUpperCase()}
            </span>
            {p.name}
            <button
              type="button"
              onClick={() => onRemoveMember(p.id)}
              aria-label={`Remove ${p.name}`}
              title={`Remove ${p.name}`}
              className="w-5 h-5 rounded-md flex items-center justify-center text-slate-500 hover:text-rose-300 hover:bg-rose-500/10 transition-colors duration-150 cursor-pointer touch-manipulation focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-500"
            >
              <svg
                className="w-3 h-3"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth="2.5"
                aria-hidden="true"
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </span>
        ))}
      </div>

      {/* Active User Summary Bar */}
      {activeParticipants.length > 0 && (
        <div className="pt-3 border-t border-slate-800/80 flex flex-col md:flex-row md:items-center justify-between gap-3 text-xs">
          <div className="flex items-center gap-2">
            <span className="text-slate-400 font-medium">Viewing as:</span>
            <select
              aria-label="Select active participant"
              value={currentUserId || ""}
              onChange={(e) => onSelectCurrentUser(e.target.value)}
              className="px-2.5 py-1.5 rounded-xl bg-slate-950 text-slate-100 border border-slate-700/80 font-semibold focus:outline-none focus:ring-2 focus:ring-indigo-500 transition cursor-pointer text-xs"
            >
              {activeParticipants.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>

          <div className="flex items-center gap-3.5 flex-wrap">
            <div>
              <span className="text-slate-400">Your expenses: </span>
              <strong className="text-white font-mono tabular-nums font-semibold">
                {formatMoney(userShareCents, group.currency)}
              </strong>
            </div>

            <span aria-hidden="true" className="text-slate-700 hidden sm:inline">
              •
            </span>

            <div>
              <span className="text-slate-400">Paid by you: </span>
              <strong className="text-white font-mono tabular-nums font-semibold">
                {formatMoney(userPaidCents, group.currency)}
              </strong>
            </div>

            <span aria-hidden="true" className="text-slate-700 hidden sm:inline">
              •
            </span>

            <div className="flex items-center gap-1.5">
              <span className="text-slate-400">Net: </span>
              <span
                className={`px-2 py-0.5 rounded-md font-mono tabular-nums font-bold ${
                  userNetCents > 0
                    ? "bg-emerald-500/15 text-emerald-300 border border-emerald-500/30"
                    : userNetCents < 0
                      ? "bg-rose-500/15 text-rose-300 border border-rose-500/30"
                      : "bg-slate-800 text-slate-300 border border-slate-700"
                }`}
              >
                {userNetCents > 0 ? "+" : ""}
                {formatMoney(userNetCents, group.currency)}
              </span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
