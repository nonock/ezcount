import { Amount } from "@/components/common/Amount";
import { HelpPopover } from "@/components/common/HelpPopover";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import type { Group, ParticipantBalance } from "@/types";
import type React from "react";
import { useMemo } from "react";

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
    <div className="space-y-3">
      <div className="flex items-center gap-1">
        <h2 className="font-medium">Who owes what</h2>
        <HelpPopover title="Balances">
          Positive amounts in <span className="font-medium text-positive">green</span> mean the
          participant is owed money back. Negative amounts in{" "}
          <span className="font-medium text-negative">red</span> mean they need to pay.
        </HelpPopover>
      </div>

      <ul className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {balances.map((b) => {
          const isPositive = b.net_cents > 0;
          const isNegative = b.net_cents < 0;
          const percentage = Math.min(100, Math.round((Math.abs(b.net_cents) / maxAbs) * 100));

          return (
            <li key={b.participant_id} data-testid="balance-card">
              <Card size="sm" className="h-full">
                <CardContent className="space-y-3">
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-2.5">
                      <Avatar>
                        <AvatarFallback>
                          {b.participant_name.charAt(0).toUpperCase()}
                        </AvatarFallback>
                      </Avatar>
                      <div className="min-w-0">
                        <h3 className="flex items-center gap-1.5 truncate font-medium">
                          {b.participant_name}
                          {b.removed && <Badge variant="secondary">Removed</Badge>}
                        </h3>
                        <p className="text-xs text-muted-foreground">
                          {isPositive ? "Gets back" : isNegative ? "Owes" : "Settled up"}
                        </p>
                      </div>
                    </div>
                    <Amount
                      cents={b.net_cents}
                      currency={group.currency}
                      tone="balance"
                      className="font-semibold"
                    />
                  </div>

                  <Progress
                    value={percentage}
                    aria-label={`${b.participant_name}'s share of the largest balance`}
                    className={cn(
                      "h-1.5",
                      isPositive && "**:data-[slot=progress-indicator]:bg-positive",
                      isNegative && "**:data-[slot=progress-indicator]:bg-negative"
                    )}
                  />

                  <div className="flex justify-between text-xs text-muted-foreground">
                    <span>
                      Paid:{" "}
                      <Amount
                        cents={b.paid_cents}
                        currency={group.currency}
                        className="text-foreground"
                      />
                    </span>
                    <span>
                      Consumed:{" "}
                      <Amount
                        cents={b.owed_cents}
                        currency={group.currency}
                        className="text-foreground"
                      />
                    </span>
                  </div>
                </CardContent>

                {isNegative && (
                  <CardFooter className="mt-auto">
                    <Button
                      variant="outline"
                      className="w-full"
                      onClick={() =>
                        onReimburseParticipant(
                          b.participant_id,
                          (Math.abs(b.net_cents) / 100).toFixed(2)
                        )
                      }
                    >
                      Reimburse debt
                      <Amount
                        cents={Math.abs(b.net_cents)}
                        currency={group.currency}
                        className="text-muted-foreground"
                      />
                    </Button>
                  </CardFooter>
                )}
              </Card>
            </li>
          );
        })}
      </ul>
    </div>
  );
};
