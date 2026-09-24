import { Amount } from "@/components/common/Amount";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { Expense, Participant } from "@/types";
import { formatDateTime } from "@/utils/formatters";
import type React from "react";

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

  const nameOf = (id: string) => participants.find((p) => p.id === id)?.name || "Unknown";
  const historyEntries = expense.history ? [...expense.history].reverse() : [];

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Expense Revision History</DialogTitle>
          <DialogDescription>
            {historyEntries.length > 0
              ? `Last modified ${formatDateTime(expense.updated_at)}`
              : `Created ${formatDateTime(expense.created_at)}`}
          </DialogDescription>
        </DialogHeader>

        <section aria-label="Current version" className="space-y-2 rounded-lg border p-3">
          <div className="flex items-start justify-between gap-2">
            <div>
              <Badge variant="secondary" className="mb-1.5">
                Current version
              </Badge>
              <p className="font-medium">{expense.title}</p>
              <p className="text-sm text-muted-foreground">Paid by {nameOf(expense.paid_by)}</p>
            </div>
            <Amount cents={expense.amount_cents} currency={currency} className="font-semibold" />
          </div>
          <div className="flex flex-wrap gap-1">
            {expense.splits.map((s) => (
              <Badge key={s.participant_id} variant="outline">
                {nameOf(s.participant_id)} ({s.shares} {s.shares === 1 ? "part" : "parts"})
              </Badge>
            ))}
          </div>
        </section>

        <section className="space-y-3">
          <h3 className="text-sm font-medium text-muted-foreground">
            Change history ({historyEntries.length} {historyEntries.length === 1 ? "edit" : "edits"}
            )
          </h3>

          {historyEntries.length === 0 ? (
            <p className="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground">
              This expense has not been modified since creation.
            </p>
          ) : (
            <ol className="relative space-y-3 border-l pl-5">
              {historyEntries.map((entry, idx) => (
                <li key={`${entry.edited_at}-${idx}`} className="relative">
                  <span
                    aria-hidden
                    className="absolute top-1.5 -left-[25px] size-2.5 rounded-full bg-primary ring-4 ring-background"
                  />
                  <div className="space-y-2 rounded-lg border p-3 text-sm">
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-medium">{formatDateTime(entry.edited_at)}</span>
                      <Badge variant="outline">Revision #{historyEntries.length - idx}</Badge>
                    </div>
                    <p>{entry.summary}</p>
                    <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 rounded-md bg-muted/50 p-2 text-xs">
                      <dt className="col-span-2 font-medium">Before this edit</dt>
                      <dt className="text-muted-foreground">Title</dt>
                      <dd className="text-right">{entry.previous_title}</dd>
                      <dt className="text-muted-foreground">Amount</dt>
                      <dd className="text-right">
                        <Amount cents={entry.previous_amount_cents} currency={currency} />
                      </dd>
                      <dt className="text-muted-foreground">Payer</dt>
                      <dd className="text-right">{nameOf(entry.previous_paid_by)}</dd>
                      <dt className="text-muted-foreground">Split</dt>
                      <dd className="text-right">
                        {entry.previous_splits
                          .map((s) => `${nameOf(s.participant_id)} (${s.shares}p)`)
                          .join(", ")}
                      </dd>
                    </dl>
                  </div>
                </li>
              ))}
            </ol>
          )}
        </section>
      </DialogContent>
    </Dialog>
  );
};
