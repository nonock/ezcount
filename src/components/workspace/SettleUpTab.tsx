import { Amount } from "@/components/common/Amount";
import { HelpPopover } from "@/components/common/HelpPopover";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";
import type { Group, SettlementTransfer } from "@/types";
import { CheckIcon, CircleCheckBigIcon, PlusIcon } from "lucide-react";
import type React from "react";

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
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-1">
          <h2 className="font-medium">Suggested payments</h2>
          <HelpPopover title="Optimal settlement plan">
            The fewest direct payments that settle every debt in the group. Mark one as paid once
            the money is sent.
          </HelpPopover>
        </div>
        {settlements.length > 0 && group.expenses.length > 0 && (
          <Button onClick={onOpenReimburse}>
            <PlusIcon data-icon="inline-start" />
            Record Reimbursement
          </Button>
        )}
      </div>

      {settlements.length === 0 ? (
        <Empty className="border border-dashed">
          <EmptyHeader>
            <EmptyMedia variant="icon" className="text-positive">
              <CircleCheckBigIcon />
            </EmptyMedia>
            <EmptyTitle>All settled up!</EmptyTitle>
            <EmptyDescription>No one in this group owes anything to anyone.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ul className="space-y-2">
          {settlements.map((s) => (
            <li key={`${s.from_id}-${s.to_id}`}>
              <Item variant="outline">
                <ItemMedia>
                  <Avatar>
                    <AvatarFallback>{s.from_name.charAt(0).toUpperCase()}</AvatarFallback>
                  </Avatar>
                </ItemMedia>
                <ItemContent>
                  <ItemTitle>
                    <span>
                      <span className="text-negative">{s.from_name}</span> pays{" "}
                      <span className="text-positive">{s.to_name}</span>
                    </span>
                  </ItemTitle>
                  <ItemDescription>Direct reimbursement</ItemDescription>
                </ItemContent>
                <ItemActions>
                  <Amount
                    cents={s.amount_cents}
                    currency={group.currency}
                    className="font-semibold"
                  />
                  <Button
                    variant="outline"
                    onClick={() =>
                      onMarkAsPaid(s.from_id, s.to_id, (s.amount_cents / 100).toFixed(2))
                    }
                  >
                    <CheckIcon data-icon="inline-start" />
                    Mark as Paid
                  </Button>
                </ItemActions>
              </Item>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};
