// What a group spent, as the Stats tab shows it: by category, by person and by month.
// Payments between members aren't spending, nor is money that came in.

import type { Expense, Group, Participant } from "@/types";
import { CATEGORIES, categoryOf } from "./categories";
import { i18n } from "./i18n/index.svelte";
import { isSpending, owedAmounts, paidAmounts } from "./split";

/** The group's expenses that are spending. */
export function spending(group: Group): Expense[] {
  return group.expenses.filter(isSpending);
}

/** The money that came in, shown apart. */
export function incomeCents(group: Group): number {
  return group.expenses.filter((e) => e.income).reduce((sum, e) => sum + e.amount_cents, 0);
}

export interface CategoryTotal {
  /** The category's key, "" for the expenses without one. */
  key: string;
  cents: number;
  count: number;
}

/** What each category cost, the biggest first; equal ones in the order they are offered. */
export function byCategory(expenses: Expense[]): CategoryTotal[] {
  const totals = new Map<string, { cents: number; count: number }>();
  for (const e of expenses) {
    const key = categoryOf(e.category)?.key ?? "";
    const total = totals.get(key) ?? { cents: 0, count: 0 };
    total.cents += e.amount_cents;
    total.count += 1;
    totals.set(key, total);
  }
  return [...CATEGORIES.map((c) => c.key as string), ""]
    .filter((key) => totals.has(key))
    .map((key) => ({ key, ...(totals.get(key) ?? { cents: 0, count: 0 }) }))
    .sort((a, b) => b.cents - a.cents);
}

export type PersonTotal = Participant & { share: number; paid: number };

/**
 * Each person's share of the spending and what they paid of it, the biggest share first.
 * Someone who left is listed while they have a part in it.
 */
export function byPerson(group: Group, expenses: Expense[]): PersonTotal[] {
  const totals = new Map(group.participants.map((p) => [p.id, { share: 0, paid: 0 }]));
  const of = (id: string) => {
    let total = totals.get(id);
    if (!total) {
      total = { share: 0, paid: 0 };
      totals.set(id, total);
    }
    return total;
  };
  for (const e of expenses) {
    const owed = owedAmounts(e.amount_cents, e.original?.amount_cents, e.splits);
    e.splits.forEach((s, i) => {
      of(s.participant_id).share += owed[i];
    });
    for (const paid of paidAmounts(e)) of(paid.id).paid += paid.cents;
  }
  return group.participants
    .map((p) => ({ ...p, ...of(p.id) }))
    .filter((p) => !p.removed || p.share > 0 || p.paid > 0)
    .sort((a, b) => b.share - a.share);
}

export interface MonthTotal {
  /** `2026-03` */
  key: string;
  /** The month in the app's language: "March 2026". */
  name: string;
  cents: number;
  /** Its bar, as a percentage of the month that cost the most. */
  width: number;
}

/** What each calendar month cost, the latest first. */
export function byMonth(expenses: Expense[]): MonthTotal[] {
  const totals = new Map<string, number>();
  for (const e of expenses) {
    const date = new Date(e.created_at);
    const key = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
    totals.set(key, (totals.get(key) ?? 0) + e.amount_cents);
  }
  const months = [...totals].sort(([a], [b]) => b.localeCompare(a));
  const most = Math.max(...months.map(([, cents]) => cents), 1);
  return months.map(([key, cents]) => {
    const [year, month] = key.split("-").map(Number);
    const name = new Date(year, month - 1, 1).toLocaleDateString(i18n.locale, {
      month: "long",
      year: "numeric",
    });
    return { key, name, cents, width: (cents / most) * 100 };
  });
}
