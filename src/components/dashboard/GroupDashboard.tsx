import { Amount } from "@/components/common/Amount";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import type { Group } from "@/types";
import { ChevronRightIcon, LinkIcon, PlusIcon, UsersIcon } from "lucide-react";
import type React from "react";

interface GroupDashboardProps {
  groups: Group[];
  onSelectGroup: (groupId: string) => void;
  onOpenCreateGroup: () => void;
  onOpenJoinGroup: () => void;
}

export const GroupDashboard: React.FC<GroupDashboardProps> = ({
  groups,
  onSelectGroup,
  onOpenCreateGroup,
  onOpenJoinGroup,
}) => {
  if (groups.length === 0) {
    return (
      <Empty className="border border-dashed py-16">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <UsersIcon />
          </EmptyMedia>
          <EmptyTitle>No groups yet</EmptyTitle>
          <EmptyDescription>
            Create a group for your next trip, dinner, flatshare or event to start splitting bills,
            or join one a friend shared with you.
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent className="flex-row justify-center gap-2">
          <Button onClick={onOpenCreateGroup}>
            <PlusIcon data-icon="inline-start" />
            Create Group
          </Button>
          <Button variant="outline" onClick={onOpenJoinGroup}>
            <LinkIcon data-icon="inline-start" />
            Join with Code
          </Button>
        </EmptyContent>
      </Empty>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end justify-between gap-2">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Your Groups</h1>
          <p className="text-sm text-muted-foreground">
            {groups.length} active {groups.length === 1 ? "group" : "groups"}
          </p>
        </div>
        <Button variant="outline" onClick={onOpenJoinGroup}>
          <LinkIcon data-icon="inline-start" />
          Join with code
        </Button>
      </div>

      <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {groups.map((group) => {
          const totalCents = group.expenses.reduce((sum, e) => sum + e.amount_cents, 0);
          const members = group.participants.filter((p) => !p.removed).length;
          return (
            <li key={group.id}>
              <Card className="relative transition-colors has-[button:focus-visible]:ring-3 has-[button:focus-visible]:ring-ring/50 hover:bg-muted/50">
                <CardHeader>
                  <CardTitle>
                    <h2 className="truncate">
                      {/* Stretched over the whole card so any tap on it opens the group. */}
                      <button
                        type="button"
                        onClick={() => onSelectGroup(group.id)}
                        className="text-left outline-none after:absolute after:inset-0 after:rounded-xl"
                      >
                        {group.name}
                      </button>
                    </h2>
                  </CardTitle>
                  <Badge variant="secondary" className="w-fit">
                    {group.currency}
                  </Badge>
                </CardHeader>
                <CardContent className="flex items-end justify-between gap-2">
                  <div>
                    <div className="text-xs text-muted-foreground">Total spent</div>
                    <Amount
                      cents={totalCents}
                      currency={group.currency}
                      className="text-lg font-semibold"
                    />
                  </div>
                  <div className="flex items-center gap-1 text-xs text-muted-foreground">
                    {members} people · {group.expenses.length} records
                    <ChevronRightIcon className="size-4" aria-hidden />
                  </div>
                </CardContent>
              </Card>
            </li>
          );
        })}
      </ul>
    </div>
  );
};
