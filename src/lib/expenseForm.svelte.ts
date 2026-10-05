// What is typed in the expense form, what it adds up to, and the expense it makes.
// `AddExpenseModal.svelte` shows it, with the fields in `src/components/expense/`.

import { api } from "@/services/api";
import type {
  Expense,
  ExpenseInput,
  ExpenseItem,
  ExpensePayer,
  ExpenseSplit,
  Group,
  OriginalAmount,
  Participant,
} from "@/types";
import { CURRENCIES } from "@/utils/currencies";
import { amountInput, formatDateInput, formatMoney } from "@/utils/formatters";
import { t } from "./i18n/index.svelte";
import { itemsOwed, owedAmounts } from "./split";

/** A number once typed in: Svelte binds number inputs as numbers. */
export type NumberField = string | number | null;

export interface SplitItemState {
  included: boolean;
  shares: number;
  /** Owes `amount` instead of parts of the rest. */
  fixed: boolean;
  amount: NumberField;
}

/** One of the people who may have paid part of the expense. */
export interface PayerState {
  included: boolean;
  amount: NumberField;
}

/** One line of an expense as it is typed. */
export interface ItemState {
  key: number;
  name: string;
  amount: NumberField;
  people: string[];
}

let nextKey = 0;

export function newItem(people: string[], name = "", amount: NumberField = ""): ItemState {
  nextKey += 1;
  return { key: nextKey, name, amount, people };
}

/** In the "Paid by" list, after the people. */
export const SEVERAL = "several";
/** In the category list: filed under nothing. */
export const NO_CATEGORY = "";
/** In the "Repeat" list: only this once. */
export const NEVER = "";
export const REPEATS = [
  { value: NEVER, label: "expense.repeatNever" },
  { value: "week", label: "expense.repeatWeek" },
  { value: "month", label: "expense.repeatMonth" },
  { value: "year", label: "expense.repeatYear" },
] as const;

/** An amount typed in a field, in cents; zero for what isn't a number. */
export function toCents(value: NumberField): number {
  const decimal = Number.parseFloat(String(value ?? ""));
  return Number.isNaN(decimal) ? 0 : Math.round(decimal * 100);
}

/** The day picked in a date field (`YYYY-MM-DD`), at the time it is now. */
export function atThisTime(day: string): string | null {
  if (!day) return null;
  const [year, month, date] = day.split("-").map(Number);
  const d = new Date();
  d.setFullYear(year, month - 1, date);
  return d.toISOString();
}

/** What the form is about. Functions, since the group changes as other devices sync. */
interface Context {
  group: () => Group;
  /** The expense being edited, if any. */
  editing: () => Expense | null;
  /** The participant the user is in the group, if they said. */
  me: () => string | null;
}

export class ExpenseForm {
  #context: Context;
  // Only the last lookup's answer is used.
  #rateLookup = 0;

  title = $state("");
  /** Money that came in rather than out. An expense stays what it was made as. */
  income = $state(false);
  repeat = $state<string>(NEVER);
  category = $state(NO_CATEGORY);
  /** In `currency`. */
  amountStr = $state<NumberField>("");
  currency = $state("");
  /** Units of the group's currency for one of `currency`. */
  rateStr = $state<NumberField>("");
  /** Typed by the user, or saved with the expense: a suggestion never replaces it. */
  rateIsOwn = $state(false);
  lookingUpRate = $state(false);
  paidBy = $state("");
  /** Several people paid: who, and how much each, in `currency`. */
  severalPayers = $state(false);
  payersState = $state<Record<string, PayerState>>({});
  expenseDate = $state(formatDateInput());
  splitsState = $state<Record<string, SplitItemState>>({});
  /** Entered line by line, as on a receipt: who owes what comes from the lines. */
  byItems = $state(false);
  itemsState = $state<ItemState[]>([]);

  constructor(context: Context) {
    this.#context = context;
    this.currency = context.group().currency;
  }

  get group(): Group {
    return this.#context.group();
  }

  get editing(): Expense | null {
    return this.#context.editing();
  }

  get me(): string | null {
    return this.#context.me();
  }

  /** Removed members stay selectable on expenses they are already part of. */
  readonly participants: Participant[] = $derived.by(() => {
    const editing = this.editing;
    const involved = new Set(
      editing
        ? [
            editing.paid_by,
            ...(editing.payers ?? []).map((p) => p.participant_id),
            ...editing.splits.map((s) => s.participant_id),
          ]
        : []
    );
    return this.group.participants.filter((p) => !p.removed || involved.has(p.id));
  });

  /** The group's currency first, then the usual ones and the one this expense is already in. */
  readonly currencies = $derived([
    ...new Set([
      this.group.currency,
      ...CURRENCIES,
      ...(this.editing?.original ? [this.editing.original.currency] : []),
    ]),
  ]);

  readonly includedParticipants = $derived(
    this.participants.filter((p) => this.splitsState[p.id]?.included)
  );
  readonly allIncluded = $derived(this.includedParticipants.length === this.participants.length);

  readonly foreign = $derived(this.currency !== this.group.currency);
  /** What was paid, in `currency`. */
  readonly paidCents = $derived(toCents(this.amountStr));
  readonly rate = $derived(Number.parseFloat(String(this.rateStr ?? "").replace(",", ".")));
  /** The same in the group's currency. */
  readonly amountCents = $derived(
    this.foreign ? (this.rate > 0 ? Math.round(this.paidCents * this.rate) : 0) : this.paidCents
  );

  readonly payers: ExpensePayer[] = $derived(
    this.participants
      .filter((p) => this.payersState[p.id]?.included)
      .map((p) => ({ participant_id: p.id, amount_cents: toCents(this.payersState[p.id].amount) }))
  );

  readonly splits: ExpenseSplit[] = $derived(
    this.includedParticipants.map((p) => {
      const state = this.splitsState[p.id];
      return state.fixed
        ? { participant_id: p.id, shares: 0, fixed_cents: toCents(state.amount) }
        : { participant_id: p.id, shares: state.shares, fixed_cents: null };
    })
  );
  readonly items: ExpenseItem[] = $derived(
    this.itemsState.map((item) => ({
      name: item.name.trim(),
      amount_cents: toCents(item.amount),
      participants: item.people,
    }))
  );
  readonly itemsCents = $derived(this.items.reduce((sum, item) => sum + item.amount_cents, 0));
  /** What each person owes of the lines, in `currency`. */
  readonly itemsOwe = $derived(itemsOwed(this.items));
  readonly totalShares = $derived(this.splits.reduce((sum, s) => sum + s.shares, 0));
  readonly fixedCents = $derived(this.splits.reduce((sum, s) => sum + (s.fixed_cents ?? 0), 0));
  readonly anyFixed = $derived(this.splits.some((s) => s.fixed_cents != null));
  /** What the parts share, in `currency`. Negative when the fixed amounts are too much. */
  readonly restCents = $derived(this.paidCents - this.fixedCents);
  /** What each included person owes, in `currency`. */
  readonly owed = $derived.by(() => {
    const amounts = owedAmounts(this.paidCents, null, this.splits);
    return new Map(this.splits.map((s, i) => [s.participant_id, amounts[i]]));
  });

  /** Why the split doesn't work out, as soon as it shows. */
  readonly splitProblem = $derived.by(() => {
    if (!this.anyFixed || this.paidCents <= 0) return null;
    if (this.restCents < 0) {
      return t("expense.tooMuch", formatMoney(-this.restCents, this.currency));
    }
    if (this.totalShares === 0 && this.restCents > 0) {
      return t("expense.leftToAssign", formatMoney(this.restCents, this.currency));
    }
    return null;
  });

  /** With several payers the amount is theirs, and the lines have to add up to it. */
  readonly itemsMismatch = $derived(
    t(
      "expense.itemsMismatch",
      formatMoney(this.itemsCents, this.currency),
      formatMoney(this.paidCents, this.currency)
    )
  );

  /** Empties the form for a new expense, or fills it with the one being edited. */
  reset() {
    const { editing, group, participants } = this;
    this.#rateLookup += 1;
    this.lookingUpRate = false;
    const next: Record<string, SplitItemState> = {};
    const nextPayers: Record<string, PayerState> = {};
    const paidTogether = editing?.payers ?? [];
    this.severalPayers = paidTogether.length > 1;
    for (const p of participants) {
      const paid = paidTogether.find((x) => x.participant_id === p.id);
      nextPayers[p.id] = {
        included: !!paid,
        amount: paid ? amountInput(paid.amount_cents) : "",
      };
    }
    this.payersState = nextPayers;
    this.income = editing?.income ?? false;
    const lines = editing?.items ?? [];
    this.byItems = lines.length > 0;
    this.itemsState = lines.map((item) =>
      newItem([...item.participants], item.name, amountInput(item.amount_cents))
    );
    this.repeat = NEVER;
    if (editing) {
      this.category = editing.category ?? NO_CATEGORY;
      this.title = editing.title;
      this.amountStr = amountInput(editing.original?.amount_cents ?? editing.amount_cents);
      this.currency = editing.original?.currency ?? group.currency;
      this.rateStr = editing.original?.rate ?? "";
      this.rateIsOwn = !!editing.original;
      this.paidBy = editing.paid_by;
      this.expenseDate = formatDateInput(editing.created_at);
      for (const p of participants) {
        const match = editing.splits.find((s) => s.participant_id === p.id);
        const fixed = match?.fixed_cents ?? null;
        next[p.id] = {
          included: !!match,
          shares: match?.shares || 1,
          fixed: fixed !== null,
          amount: fixed !== null ? amountInput(fixed) : "",
        };
      }
    } else {
      const me = this.me;
      this.category = NO_CATEGORY;
      this.title = "";
      this.amountStr = "";
      this.currency = group.currency;
      this.rateStr = "";
      this.rateIsOwn = false;
      this.expenseDate = formatDateInput();
      this.paidBy = me && participants.some((p) => p.id === me) ? me : participants[0]?.id || "";
      for (const p of participants) {
        next[p.id] = { included: true, shares: 1, fixed: false, amount: "" };
      }
    }
    this.splitsState = next;
  }

  /**
   * Fills in the relay's rate for the expense's day, or else the one last used in the group
   * for that currency. Only suggests: a rate that is the user's own stays.
   */
  async suggestRate() {
    const group = this.group;
    if (this.currency === group.currency || this.rateIsOwn) return;
    this.#rateLookup += 1;
    const lookup = this.#rateLookup;
    const from = this.currency;
    this.lookingUpRate = true;
    let suggestion: string | null = null;
    try {
      suggestion = await api.suggestExchangeRate(from, group.currency, this.expenseDate || null);
    } catch (err) {
      console.warn("No exchange rate from the relay:", err);
    }
    if (lookup !== this.#rateLookup) return;
    this.lookingUpRate = false;
    if (this.rateIsOwn) return;
    const last = group.expenses.filter((e) => e.original?.currency === from).at(-1);
    this.rateStr = suggestion ?? last?.original?.rate ?? "";
  }

  chooseCurrency(next: string) {
    this.currency = next;
    this.rateStr = "";
    this.rateIsOwn = false;
    this.#rateLookup += 1;
    this.lookingUpRate = false;
    this.suggestRate();
  }

  /** The rate follows the day, as long as it is a suggestion. */
  chooseDate(next: string) {
    this.expenseDate = next;
    this.suggestRate();
  }

  payerOf(id: string): PayerState {
    this.payersState[id] ??= { included: false, amount: "" };
    return this.payersState[id];
  }

  choosePayer(value: string) {
    if (value !== SEVERAL) {
      this.severalPayers = false;
      this.paidBy = value;
      return;
    }
    this.severalPayers = true;
    // Starts from who was paying alone, with the whole amount.
    if (!this.participants.some((p) => this.payersState[p.id]?.included) && this.paidBy) {
      const state = this.payerOf(this.paidBy);
      state.included = true;
      state.amount = this.paidCents > 0 ? amountInput(this.paidCents) : "";
    }
  }

  togglePayer(id: string) {
    const state = this.payerOf(id);
    state.included = !state.included;
    state.amount = "";
    this.totalPayers();
  }

  /** With several payers, the expense's amount is what they paid between them. */
  totalPayers() {
    const total = this.participants
      .filter((p) => this.payersState[p.id]?.included)
      .reduce((sum, p) => sum + toCents(this.payersState[p.id].amount), 0);
    this.amountStr = total > 0 ? amountInput(total) : "";
  }

  /** With lines, the expense's amount is their total, unless several payers already give it. */
  totalItems() {
    if (this.severalPayers) return;
    this.amountStr = this.itemsCents > 0 ? amountInput(this.itemsCents) : "";
  }

  /** The first line starts as the whole expense, for everyone it was split between. */
  toggleItems() {
    this.byItems = !this.byItems;
    if (!this.byItems) return;
    if (this.itemsState.length === 0) {
      const people = this.includedParticipants.map((p) => p.id);
      const amount = this.paidCents > 0 ? amountInput(this.paidCents) : "";
      this.itemsState.push(newItem(people, "", amount));
    }
    this.totalItems();
  }

  /** Also for someone who joined the group while the dialog is open. */
  stateOf(id: string): SplitItemState {
    this.splitsState[id] ??= { included: false, shares: 1, fixed: false, amount: "" };
    return this.splitsState[id];
  }

  toggleParticipant(id: string) {
    const state = this.stateOf(id);
    state.included = !state.included;
  }

  updateShares(id: string, shares: number) {
    if (shares >= 1) this.stateOf(id).shares = shares;
  }

  toggleAll() {
    const include = !this.allIncluded;
    for (const p of this.participants) this.stateOf(p.id).included = include;
  }

  /** The expense as typed, or what is missing for it to be saved. */
  result(): { input: ExpenseInput; error?: undefined } | { error: string } {
    const { group, currency, payers, items, splits, paidCents } = this;
    const title = this.title.trim();
    if (!title) return { error: t("expense.needDescription") };
    if (this.severalPayers && payers.length === 0) return { error: t("expense.needPayers") };
    // One person ticked among "several" paid it all.
    const together = this.severalPayers && payers.length > 1 ? payers : [];
    if (together.some((p) => p.amount_cents <= 0)) return { error: t("expense.needPayerAmounts") };
    if (this.byItems && items.length === 0) return { error: t("expense.needItems") };
    if (this.byItems && items.some((item) => item.amount_cents <= 0)) {
      return { error: t("expense.needItemAmounts") };
    }
    if (paidCents <= 0) return { error: t("expense.needAmount") };
    if (this.foreign && !(this.rate > 0)) {
      return { error: t("expense.needRate", currency, group.currency) };
    }
    if (this.amountCents <= 0) return { error: t("expense.lessThanCent", group.currency) };
    if (this.byItems) {
      if (items.some((item) => item.participants.length === 0)) {
        return { error: t("expense.needItemPeople") };
      }
      if (this.itemsCents !== paidCents) return { error: this.itemsMismatch };
    } else {
      if (this.includedParticipants.length === 0) return { error: t("expense.needPeople") };
      if (splits.some((s) => s.fixed_cents != null && s.fixed_cents <= 0)) {
        return { error: t("expense.needSetAmounts") };
      }
      if (this.splitProblem) return { error: this.splitProblem };
    }

    const original: OriginalAmount | null = this.foreign
      ? { currency, amount_cents: paidCents, rate: String(this.rateStr).trim() }
      : null;
    return {
      input: {
        title,
        category: this.category || null,
        amount_cents: this.amountCents,
        paid_by: this.severalPayers ? payers[0].participant_id : this.paidBy,
        payers: together,
        // The lines give the splits.
        splits: this.byItems ? [] : splits,
        items: this.byItems ? items : [],
        created_at: atThisTime(this.expenseDate),
        original,
        income: this.income,
        // The rate of another currency would be the first day's for good.
        repeat: !this.editing && !this.foreign && this.repeat !== NEVER ? this.repeat : null,
      },
    };
  }
}
