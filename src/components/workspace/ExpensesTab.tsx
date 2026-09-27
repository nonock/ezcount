import { Amount } from "@/components/common/Amount";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Empty,
  EmptyContent,
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
import type { Expense, Group } from "@/types";
import {
  formatDate,
  formatDateGroupHeader,
  formatMoney,
  getLocalDateKey,
} from "@/utils/formatters";
import {
  ArrowRightIcon,
  CheckIcon,
  EllipsisIcon,
  HandCoinsIcon,
  HistoryIcon,
  PencilIcon,
  PlusIcon,
  ReceiptTextIcon,
  Trash2Icon,
} from "lucide-react";
import type React from "react";
import { useEffect, useMemo, useRef, useState } from "react";

interface ExpensesTabProps {
  group: Group;
  hasOutstandingDebt?: boolean;
  onOpenAddExpense: () => void;
  onOpenReimburse: () => void;
  onDeleteExpense: (expenseId: string) => void;
  onEditExpense: (expense: Expense) => void;
  onViewHistory: (expense: Expense) => void;
}

interface DateGroup {
  dateKey: string;
  displayDate: string;
  totalCents: number;
  items: Expense[];
}

const PAGE_SIZE = 10;

export const ExpensesTab: React.FC<ExpensesTabProps> = ({
  group,
  hasOutstandingDebt = false,
  onOpenAddExpense,
  onOpenReimburse,
  onDeleteExpense,
  onEditExpense,
  onViewHistory,
}) => {
  const nameMap = useMemo(
    () => new Map(group.participants.map((p) => [p.id, p.name])),
    [group.participants]
  );
  const nameOf = (id: string | undefined) => (id && nameMap.get(id)) || "Unknown";

  // Newest first
  const sortedExpenses = useMemo(() => {
    return [...group.expenses].sort((a, b) => {
      const timeA = new Date(a.created_at).getTime() || 0;
      const timeB = new Date(b.created_at).getTime() || 0;
      return timeB - timeA;
    });
  }, [group.expenses]);

  // Infinite loading, PAGE_SIZE at a time
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (group.id) {
      setVisibleCount(PAGE_SIZE);
    }
  }, [group.id]);

  const visibleExpenses = useMemo(
    () => sortedExpenses.slice(0, visibleCount),
    [sortedExpenses, visibleCount]
  );

  const hasMore = visibleCount < sortedExpenses.length;
  const remainingCount = sortedExpenses.length - visibleCount;

  const handleLoadMore = () => {
    setVisibleCount((prev) => Math.min(prev + PAGE_SIZE, sortedExpenses.length));
  };

  useEffect(() => {
    if (!hasMore) return;
    const sentinel = sentinelRef.current;
    if (!sentinel) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          setVisibleCount((prev) => Math.min(prev + PAGE_SIZE, sortedExpenses.length));
        }
      },
      { rootMargin: "250px" }
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMore, sortedExpenses.length]);

  // Group visible expenses by local calendar day
  const dateGroups = useMemo(() => {
    const map = new Map<string, DateGroup>();
    for (const exp of visibleExpenses) {
      const key = getLocalDateKey(exp.created_at);
      const existing = map.get(key);
      if (existing) {
        existing.items.push(exp);
        if (!exp.is_reimbursement) {
          existing.totalCents += exp.amount_cents;
        }
      } else {
        map.set(key, {
          dateKey: key,
          displayDate: formatDateGroupHeader(exp.created_at),
          totalCents: exp.is_reimbursement ? 0 : exp.amount_cents,
          items: [exp],
        });
      }
    }
    return Array.from(map.values());
  }, [visibleExpenses]);

  const totalCents = useMemo(
    () => group.expenses.reduce((sum, e) => sum + e.amount_cents, 0),
    [group.expenses]
  );

  return (
    // On phones, room below the list for the floating Add Expense button.
    <div className="space-y-5 max-sm:pb-16">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="flex items-center gap-2 font-semibold">
            Transaction History <Badge variant="secondary">{group.expenses.length}</Badge>
          </h2>
          <p className="text-sm text-muted-foreground">
            Total recorded volume: <Amount cents={totalCents} currency={group.currency} />
          </p>
        </div>

        <div className="flex items-center gap-2">
          {hasOutstandingDebt && (
            <Button variant="outline" onClick={onOpenReimburse}>
              <CheckIcon data-icon="inline-start" />
              Reimburse
            </Button>
          )}
          {/* On phones, floating above the bottom bar, within reach of the thumb. */}
          <Button
            onClick={onOpenAddExpense}
            className="max-sm:fixed max-sm:right-4 max-sm:bottom-[calc(5rem+env(safe-area-inset-bottom))] max-sm:z-30 max-sm:h-12 max-sm:rounded-full max-sm:px-5 max-sm:text-base max-sm:shadow-lg"
          >
            <PlusIcon data-icon="inline-start" />
            Add Expense
          </Button>
        </div>
      </div>

      {group.expenses.length === 0 && (
        <Empty className="border border-dashed">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ReceiptTextIcon />
            </EmptyMedia>
            <EmptyTitle>No expenses recorded yet</EmptyTitle>
            <EmptyDescription>
              Add your first shared expense or bill to start calculating fair balances.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button variant="outline" onClick={onOpenAddExpense}>
              <PlusIcon data-icon="inline-start" />
              Add First Expense
            </Button>
          </EmptyContent>
        </Empty>
      )}

      {dateGroups.map((dg) => (
        <section
          key={dg.dateKey}
          aria-labelledby={`date-header-${dg.dateKey}`}
          className="space-y-2"
        >
          <div
            id={`date-header-${dg.dateKey}`}
            className="flex items-center justify-between px-1 text-sm"
          >
            <span>
              <span className="font-medium">{dg.displayDate}</span>
              <span className="text-muted-foreground">
                {" "}
                · {dg.items.length} {dg.items.length === 1 ? "transaction" : "transactions"}
              </span>
            </span>
            {dg.totalCents > 0 && (
              <Amount
                cents={dg.totalCents}
                currency={group.currency}
                className="text-muted-foreground"
              />
            )}
          </div>

          <ul className="space-y-2">
            {dg.items.map((e) => {
              const isReimbursement = Boolean(e.is_reimbursement);
              const editCount = e.history?.length ?? 0;
              const totalShares = e.splits.reduce((sum, s) => sum + s.shares, 0);

              return (
                <li key={e.id} data-testid="expense-item">
                  <Item variant="outline" className="items-start sm:items-center">
                    <ItemMedia>
                      <Avatar>
                        <AvatarFallback className={isReimbursement ? "text-positive" : ""}>
                          {isReimbursement ? (
                            <HandCoinsIcon className="size-4" aria-hidden />
                          ) : (
                            e.title.charAt(0).toUpperCase()
                          )}
                        </AvatarFallback>
                      </Avatar>
                    </ItemMedia>

                    <ItemContent className="min-w-0">
                      <ItemTitle className="flex-wrap">
                        <h3 className="truncate">{e.title}</h3>
                        {isReimbursement && (
                          <Badge variant="outline" className="text-positive">
                            Reimbursement
                          </Badge>
                        )}
                        {editCount > 0 && (
                          <Badge asChild variant="secondary">
                            <button
                              type="button"
                              onClick={() => onViewHistory(e)}
                              aria-label={`Edited (${editCount}): View revision history for ${e.title}`}
                            >
                              <HistoryIcon /> Edited ({editCount})
                            </button>
                          </Badge>
                        )}
                      </ItemTitle>

                      {isReimbursement ? (
                        <ItemDescription className="flex items-center gap-1">
                          <span>Paid by</span>
                          <span className="font-medium text-foreground">{nameOf(e.paid_by)}</span>
                          <ArrowRightIcon className="size-3.5" aria-hidden />
                          <span className="sr-only">to</span>
                          <span className="font-medium text-foreground">
                            {nameOf(e.splits[0]?.participant_id)}
                          </span>
                        </ItemDescription>
                      ) : (
                        <>
                          <ItemDescription>
                            Paid by{" "}
                            <span className="font-medium text-foreground">{nameOf(e.paid_by)}</span>
                          </ItemDescription>
                          <div className="flex flex-wrap gap-1 pt-1" aria-label="Split between">
                            {e.splits.map((s) => (
                              <Badge key={s.participant_id} variant="secondary">
                                {nameOf(s.participant_id)}
                                {s.shares > 1 && ` ×${s.shares}`}
                              </Badge>
                            ))}
                          </div>
                        </>
                      )}
                      <span className="text-xs text-muted-foreground">
                        {formatDate(e.created_at)}
                      </span>
                    </ItemContent>

                    <ItemActions>
                      <div className="text-right">
                        <Amount
                          cents={e.amount_cents}
                          currency={group.currency}
                          tone={isReimbursement ? "positive" : "neutral"}
                          className="block font-semibold"
                        />
                        {!isReimbursement && (
                          <span className="block text-xs text-muted-foreground tabular-nums">
                            {formatMoney(
                              Math.floor(e.amount_cents / (totalShares || 1)),
                              group.currency
                            )}{" "}
                            / part
                          </span>
                        )}
                      </div>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button variant="ghost" size="icon" aria-label={`Actions for ${e.title}`}>
                            <EllipsisIcon />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem onSelect={() => onEditExpense(e)}>
                            <PencilIcon /> Edit
                          </DropdownMenuItem>
                          {editCount > 0 && (
                            <DropdownMenuItem onSelect={() => onViewHistory(e)}>
                              <HistoryIcon /> View history
                            </DropdownMenuItem>
                          )}
                          <DropdownMenuSeparator />
                          <DropdownMenuItem
                            variant="destructive"
                            onSelect={() => onDeleteExpense(e.id)}
                          >
                            <Trash2Icon /> Delete
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </ItemActions>
                  </Item>
                </li>
              );
            })}
          </ul>
        </section>
      ))}

      {hasMore && (
        <div ref={sentinelRef} className="text-center">
          <Button variant="outline" onClick={handleLoadMore}>
            Load 10 more transactions
            <span className="text-muted-foreground">({remainingCount} remaining)</span>
          </Button>
        </div>
      )}

      {sortedExpenses.length > PAGE_SIZE && (
        <p className="text-center text-xs text-muted-foreground">
          Showing {visibleExpenses.length} of {sortedExpenses.length} transactions
        </p>
      )}
    </div>
  );
};
