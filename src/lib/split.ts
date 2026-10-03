// How an expense is divided, as `engine.rs` computes it, to show it before it is saved.

import type { Expense, ExpenseSplit } from "@/types";

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

/** The currency an expense's fixed amounts are in: the one it was paid in. */
export function paidCurrency(expense: Pick<Expense, "original">, groupCurrency: string): string {
  return expense.original?.currency ?? groupCurrency;
}
