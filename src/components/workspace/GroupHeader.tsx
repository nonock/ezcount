import { Amount } from "@/components/common/Amount";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardHeader } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import type { Group, ParticipantBalance, SyncInfo } from "@/types";
import {
  EllipsisVerticalIcon,
  LogOutIcon,
  UserPlusIcon,
  UserRoundIcon,
  UsersIcon,
  XIcon,
} from "lucide-react";
import type React from "react";
import { useMemo } from "react";

interface GroupHeaderProps {
  group: Group;
  balances: ParticipantBalance[];
  currentUserId: string | null;
  /** Opens the "Who are you?" dialog. */
  onChangeIdentity: () => void;
  onOpenAddMember: () => void;
  onRemoveMember: (participantId: string) => void;
  onOpenShare: () => void;
  onLeaveGroup: () => void;
  syncInfo: SyncInfo | null;
}

export const GroupHeader: React.FC<GroupHeaderProps> = ({
  group,
  balances,
  currentUserId,
  onChangeIdentity,
  onOpenAddMember,
  onRemoveMember,
  onOpenShare,
  onLeaveGroup,
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

  const me = group.participants.find((p) => p.id === currentUserId);
  const syncProblem = Boolean(syncInfo?.last_error);

  return (
    <Card>
      <CardHeader>
        <div className="flex min-w-0 items-center gap-2">
          <h1 className="truncate text-lg font-semibold tracking-tight">{group.name}</h1>
          <Badge variant="secondary">{group.currency}</Badge>
        </div>
        <CardDescription>
          {activeParticipants.length} participants · Group total{" "}
          <Amount cents={totalCents} currency={group.currency} className="text-foreground" />
        </CardDescription>
        <CardAction className="flex items-center gap-1">
          <Button
            variant="outline"
            onClick={onOpenShare}
            title={
              syncProblem ? `Last sync failed: ${syncInfo?.last_error}` : "Invite other members"
            }
          >
            <span
              aria-hidden
              className={cn("size-2 rounded-full", syncProblem ? "bg-negative" : "bg-positive")}
            />
            {syncProblem ? "Sync issue" : "Invite"}
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" aria-label="Group options">
                <EllipsisVerticalIcon />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onSelect={onOpenAddMember}>
                <UserPlusIcon /> Add member
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={onChangeIdentity}>
                <UserRoundIcon /> Change who you are
              </DropdownMenuItem>
              <DropdownMenuItem variant="destructive" onSelect={onLeaveGroup}>
                <LogOutIcon /> Leave group
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </CardAction>
      </CardHeader>

      <CardContent className="space-y-4">
        <ul className="flex flex-wrap gap-1.5" aria-label="Members">
          {activeParticipants.map((p) => (
            <li key={p.id}>
              <Badge variant="outline" className="h-7 gap-1 pr-0.5 pl-2.5 text-sm">
                {p.name}
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => onRemoveMember(p.id)}
                  aria-label={`Remove ${p.name}`}
                  className="text-muted-foreground hover:text-destructive"
                >
                  <XIcon />
                </Button>
              </Badge>
            </li>
          ))}
          <li>
            <Button variant="ghost" size="sm" onClick={onOpenAddMember} className="h-7">
              <UserPlusIcon data-icon="inline-start" />
              Add Member
            </Button>
          </li>
        </ul>

        {activeParticipants.length > 0 && (
          <>
            <Separator />
            <div className="flex flex-col gap-3 text-sm md:flex-row md:items-center md:justify-between">
              {me ? (
                <p className="flex items-center gap-1 text-muted-foreground">
                  You're <span className="font-medium text-foreground">{me.name}</span>
                  <Button
                    variant="link"
                    size="sm"
                    onClick={onChangeIdentity}
                    className="h-auto px-1"
                  >
                    Change
                  </Button>
                </p>
              ) : (
                <Button variant="outline" size="sm" onClick={onChangeIdentity} className="w-fit">
                  <UsersIcon data-icon="inline-start" />
                  Who are you in this group?
                </Button>
              )}

              <dl className="grid grid-cols-3 gap-3 md:flex md:gap-6">
                <div>
                  <dt className="text-xs text-muted-foreground">Your expenses</dt>
                  <dd>
                    <Amount
                      cents={currentUserBalance?.owed_cents || 0}
                      currency={group.currency}
                      className="font-medium"
                    />
                  </dd>
                </div>
                <div>
                  <dt className="text-xs text-muted-foreground">Paid by you</dt>
                  <dd>
                    <Amount
                      cents={userPaidCents}
                      currency={group.currency}
                      className="font-medium"
                    />
                  </dd>
                </div>
                <div>
                  <dt className="text-xs text-muted-foreground">Net</dt>
                  <dd>
                    <Amount
                      cents={currentUserBalance?.net_cents || 0}
                      currency={group.currency}
                      tone="balance"
                      className="font-semibold"
                    />
                  </dd>
                </div>
              </dl>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
};
