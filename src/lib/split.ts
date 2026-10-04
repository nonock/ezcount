// How an expense is divided, as `engine.rs` computes it, to show it before it is saved.

import type { Expense, ExpenseItem, ExpenseSplit, Group } from "@/types";

type Paid = Pick<Expense, "paid_by" | "payers" | "amount_cents" | "original">;

/** `amountCents` in proportion to `weights`, the leftover cents going to the largest remainders. */
function splitWeighted(amountCents: number, weights: number[]): number[] {
  const total = weights.reduce((sum, w) => sum + w, 0);
  if (total <= 0) return weights.map(() => 0);
  const parts = weights.map((w) => ({
    base: Math.floor((amountCents * w) / total),
    rem: (amountCents * w) % total,
  }));
  const leftover = amountCents - parts.reduce((sum, p) => sum + p.base, 0);
  const order = [...parts.keys()].sort((a, b) => parts[b].rem - parts[a].rem);
  for (const i of order.slice(0, leftover)) parts[i].base += 1;
  return parts.map((p) => p.base);
}

/**
 * What each split owes, in the same order. Fixed amounts come first and the parts share the
 * rest, in the currency the expense was paid in (`originalCents`, when it isn't the
 * group's); `amountCents`, in the group's currency, is then divided in the same proportions.
 */
export function owedAmounts(
  amountCents: number,
  originalCents: number | null | undefined,
  splits: ExpenseSplit[]
): number[] {
  const shares = splits.map((s) => s.shares);
  if (splits.every((s) => s.fixed_cents == null)) return splitWeighted(amountCents, shares);
  const fixed = splits.reduce((sum, s) => sum + (s.fixed_cents ?? 0), 0);
  const rest = splitWeighted(Math.max((originalCents ?? amountCents) - fixed, 0), shares);
  const there = splits.map((s, i) => s.fixed_cents ?? rest[i]);
  return originalCents == null ? there : splitWeighted(amountCents, there);
}

/**
 * Who paid an expense and how much, in the group's currency, who paid the most first: one
 * person all of it, or several their amounts. Those are in the currency paid, so an expense
 * paid in another one has its amount divided in the same proportions (as `engine::paid`).
 */
export function paidAmounts(expense: Paid): { id: string; cents: number }[] {
  const payers = expense.payers ?? [];
  if (payers.length === 0) return [{ id: expense.paid_by, cents: expense.amount_cents }];
  const there = payers.map((p) => p.amount_cents);
  const here = expense.original ? splitWeighted(expense.amount_cents, there) : there;
  return payers.map((p, i) => ({ id: p.participant_id, cents: here[i] }));
}

/**
 * What each person owes of an expense entered line by line, in the order they first appear
 * (as `doc::items_owed`): a line is shared equally between its people, the cents left over
 * going to the first ones.
 */
export function itemsOwed(
  items: Pick<ExpenseItem, "amount_cents" | "participants">[]
): Map<string, number> {
  const owed = new Map<string, number>();
  for (const item of items) {
    const people = item.participants.length;
    if (people === 0) continue;
    const each = Math.floor(item.amount_cents / people);
    const extra = item.amount_cents % people;
    item.participants.forEach((id, i) => {
      owed.set(id, (owed.get(id) ?? 0) + each + (i < extra ? 1 : 0));
    });
  }
  return owed;
}

/**
 * What the group owes a member (above zero) or the member owes it (below), as
 * `engine::calculate_balances`: money that came in counts the other way.
 */
export function netBalance(group: Group, participantId: string): number {
  let net = 0;
  for (const e of group.expenses) {
    const sign = e.income ? -1 : 1;
    for (const paid of paidAmounts(e)) {
      if (paid.id === participantId) net += sign * paid.cents;
    }
    const owed = owedAmounts(e.amount_cents, e.original?.amount_cents, e.splits);
    e.splits.forEach((s, i) => {
      if (s.participant_id === participantId) net -= sign * owed[i];
    });
  }
  return net;
}

/** The currency an expense's fixed amounts are in: the one it was paid in. */
export function paidCurrency(expense: Pick<Expense, "original">, groupCurrency: string): string {
  return expense.original?.currency ?? groupCurrency;
}
