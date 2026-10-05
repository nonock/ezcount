// The expense list's search, filters and order, and what they leave of a group's expenses.
// `ExpensesTab.svelte` shows it.

import type { Expense, Group } from "@/types";
import {
  expenseTitle,
  formatDateGroupHeader,
  formatList,
  formatMoney,
  getLocalDateKey,
} from "@/utils/formatters";
import { CATEGORIES, categoryName, categoryOf } from "./categories";
import { t } from "./i18n/index.svelte";
import { isSpending, paidAmounts } from "./split";

export type Kind = "all" | "expenses" | "income" | "payments";
export type Sort = "newest" | "oldest" | "highest" | "lowest" | "title";

export const KINDS = [
  { value: "all", label: "expenses.all" },
  { value: "expenses", label: "expenses.onlyExpenses" },
  { value: "income", label: "expenses.onlyIncome" },
  { value: "payments", label: "expenses.onlyPayments" },
] as const;
export const SORTS = [
  { value: "newest", label: "expenses.newest" },
  { value: "oldest", label: "expenses.oldest" },
  { value: "highest", label: "expenses.highest" },
  { value: "lowest", label: "expenses.lowest" },
  { value: "title", label: "expenses.byTitle" },
] as const;
export const ANYONE = "";
export const ANY_CATEGORY = "any";

/** What is shown: a search, a kind, a person, a category and an order. */
export class ExpenseFilter {
  query = $state("");
  kind = $state<Kind>("all");
  person = $state(ANYONE);
  /** A category's key, "" for the expenses without one, or any. */
  category = $state(ANY_CATEGORY);
  sort = $state<Sort>("newest");

  /** Whether some expenses may be left out. */
  get filtering(): boolean {
    return this.query.trim() !== "" || this.narrowed;
  }

  /** Whether the menu leaves some expenses out (the search is not in the menu). */
  get narrowed(): boolean {
    return this.kind !== "all" || this.person !== ANYONE || this.category !== ANY_CATEGORY;
  }

  /** By day only makes sense in date order. */
  get byDay(): boolean {
    return this.sort === "newest" || this.sort === "oldest";
  }

  /** Shows every expense again, in the order chosen. */
  clear() {
    this.query = "";
    this.kind = "all";
    this.person = ANYONE;
    this.category = ANY_CATEGORY;
  }
}

/** What narrows the list, as `matchingExpenses` reads it. */
export type Filtering = Pick<ExpenseFilter, "query" | "kind" | "person" | "category">;

export function kindOf(e: Expense): Exclude<Kind, "all"> {
  return e.is_reimbursement ? "payments" : e.income ? "income" : "expenses";
}

/** The key the filter and the statistics know an expense's category by. */
export function categoryKey(e: Expense): string | null {
  return e.is_reimbursement ? null : (categoryOf(e.category)?.key ?? "");
}

/** Lower case without accents, so "cafe" finds "Café". */
export function plain(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

/** A member's name, by their id. */
export function memberName(group: Group, id: string | undefined): string {
  return group.participants.find((p) => p.id === id)?.name || t("common.unknown");
}

/** "Alice", or "Alice and Bob" when several people paid. */
export function payersOf(group: Group, e: Expense): string {
  return formatList(paidAmounts(e).map((paid) => memberName(group, paid.id)));
}

/** Only the categories the group uses are offered, then the expenses without one. */
export function usedCategories(group: Group): { value: string; label: string }[] {
  const used = new Set(group.expenses.map(categoryKey));
  return [
    ...CATEGORIES.filter((c) => used.has(c.key)).map((c) => ({
      value: c.key as string,
      label: t(c.label),
    })),
    ...(used.has("") ? [{ value: "", label: t("category.none") }] : []),
  ];
}

/**
 * The group's expenses the filter keeps. The search looks at the title, at the category, at
 * who paid and at the amount as it is shown.
 */
export function matchingExpenses(group: Group, filter: Filtering): Expense[] {
  const { kind, person, category } = filter;
  const words = plain(filter.query).split(/\s+/).filter(Boolean);
  return group.expenses.filter((e) => {
    if (kind !== "all" && kindOf(e) !== kind) return false;
    if (
      person !== ANYONE &&
      !paidAmounts(e).some((paid) => paid.id === person) &&
      !e.splits.some((s) => s.participant_id === person)
    ) {
      return false;
    }
    if (category !== ANY_CATEGORY && categoryKey(e) !== category) return false;
    if (words.length === 0) return true;
    const text = plain(
      `${expenseTitle(e)} ${e.category ? categoryName(e.category) : ""} ${payersOf(group, e)} ${formatMoney(e.amount_cents, group.currency)} ${(e.amount_cents / 100).toFixed(2)}`
    );
    return words.every((word) => text.includes(word));
  });
}

/** The expenses in the order chosen; the latest first among those the order doesn't tell apart. */
export function sortedExpenses(expenses: Expense[], sort: Sort): Expense[] {
  const time = (e: Expense) => new Date(e.created_at).getTime() || 0;
  const newest = (a: Expense, b: Expense) => time(b) - time(a);
  const order: Record<Sort, (a: Expense, b: Expense) => number> = {
    newest,
    oldest: (a, b) => -newest(a, b),
    highest: (a, b) => b.amount_cents - a.amount_cents || newest(a, b),
    lowest: (a, b) => a.amount_cents - b.amount_cents || newest(a, b),
    title: (a, b) => expenseTitle(a).localeCompare(expenseTitle(b)) || newest(a, b),
  };
  return [...expenses].sort(order[sort]);
}

/** What the filter's expenses add up to: payments and money that came in aren't spending. */
export function spendingCents(expenses: Expense[]): number {
  return expenses.filter(isSpending).reduce((sum, e) => sum + e.amount_cents, 0);
}

export interface DateGroup {
  dateKey: string;
  displayDate: string;
  totalCents: number;
  items: Expense[];
}

/** The expenses by local calendar day, or all in one list when sorted another way. */
export function dateGroups(expenses: Expense[], byDay: boolean): DateGroup[] {
  const map = new Map<string, DateGroup>();
  for (const exp of expenses) {
    const key = byDay ? getLocalDateKey(exp.created_at) : "all";
    const existing = map.get(key);
    if (existing) {
      existing.items.push(exp);
      if (isSpending(exp)) existing.totalCents += exp.amount_cents;
    } else {
      map.set(key, {
        dateKey: key,
        displayDate: formatDateGroupHeader(exp.created_at),
        totalCents: isSpending(exp) ? exp.amount_cents : 0,
        items: [exp],
      });
    }
  }
  return Array.from(map.values());
}

/** An expense shared equally by everyone is the usual case: only the others list who shares. */
export function isForEveryone(group: Group, e: Expense): boolean {
  const activeIds = group.participants.filter((p) => !p.removed).map((p) => p.id);
  return (
    e.splits.length === activeIds.length &&
    e.splits.every(
      (s) =>
        s.fixed_cents == null &&
        s.shares === e.splits[0].shares &&
        activeIds.includes(s.participant_id)
    )
  );
}
