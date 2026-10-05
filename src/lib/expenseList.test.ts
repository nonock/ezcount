import { afterEach, describe, expect, it, vi } from "vitest";
import { equally, expense, group, participant } from "@/test/fixtures";
import {
  ANY_CATEGORY,
  ANYONE,
  categoryKey,
  dateGroups,
  ExpenseFilter,
  type Filtering,
  isForEveryone,
  kindOf,
  matchingExpenses,
  memberName,
  payersOf,
  plain,
  sortedExpenses,
  spendingCents,
  usedCategories,
} from "./expenseList.svelte";

const day = (date: number, hour = 12) => new Date(2026, 2, date, hour).toISOString();

const taxi = expense({ id: "taxi", title: "Taxi", amount_cents: 3000, created_at: day(10) });
const cafe = expense({
  id: "cafe",
  title: "Café",
  category: "food",
  amount_cents: 850,
  paid_by: "bob",
  splits: equally("bob", "carol"),
  created_at: day(12),
});
const payment = expense({
  id: "payment",
  title: "Payment: Bob → Alice",
  amount_cents: 1500,
  paid_by: "bob",
  splits: equally("alice"),
  is_reimbursement: true,
  created_at: day(11),
});
const refund = expense({
  id: "refund",
  title: "Deposit back",
  amount_cents: 3000,
  income: true,
  created_at: day(12, 9),
});
const trip = group({ expenses: [taxi, cafe, payment, refund] });

const everything: Filtering = { query: "", kind: "all", person: ANYONE, category: ANY_CATEGORY };
const ids = (filter: Partial<Filtering>) =>
  matchingExpenses(trip, { ...everything, ...filter }).map((e) => e.id);

afterEach(() => vi.useRealTimers());

describe("ExpenseFilter", () => {
  it("starts with everything, the latest first, by day", () => {
    const filter = new ExpenseFilter();
    expect(filter.filtering).toBe(false);
    expect(filter.narrowed).toBe(false);
    expect(filter.byDay).toBe(true);
  });

  it("knows a search from the menu's filters", () => {
    const filter = new ExpenseFilter();
    filter.query = "  ";
    expect(filter.filtering).toBe(false);
    filter.query = "taxi";
    expect(filter.filtering).toBe(true);
    expect(filter.narrowed).toBe(false);
    filter.person = "bob";
    expect(filter.narrowed).toBe(true);
  });

  it("lists by day only in date order", () => {
    const filter = new ExpenseFilter();
    filter.sort = "oldest";
    expect(filter.byDay).toBe(true);
    filter.sort = "highest";
    expect(filter.byDay).toBe(false);
  });

  it("clears what narrows the list and keeps the order", () => {
    const filter = new ExpenseFilter();
    Object.assign(filter, { query: "x", kind: "income", person: "bob", category: "food" });
    filter.sort = "title";
    filter.clear();
    expect(filter.filtering).toBe(false);
    expect(filter.sort).toBe("title");
  });
});

describe("what an expense is", () => {
  it("is an expense, money that came in or a payment", () => {
    expect(kindOf(taxi)).toBe("expenses");
    expect(kindOf(refund)).toBe("income");
    expect(kindOf(payment)).toBe("payments");
  });

  it("has a category key, none for a payment", () => {
    expect(categoryKey(cafe)).toBe("food");
    expect(categoryKey(taxi)).toBe("");
    expect(categoryKey(payment)).toBeNull();
    expect(categoryKey(expense({ category: "from-the-future" }))).toBe("other");
  });

  it("names its payers", () => {
    expect(payersOf(trip, taxi)).toBe("Alice");
    const together = expense({
      payers: [
        { participant_id: "alice", amount_cents: 2000 },
        { participant_id: "nobody", amount_cents: 1000 },
      ],
    });
    expect(payersOf(trip, together)).toBe("Alice and Unknown");
    expect(memberName(trip, undefined)).toBe("Unknown");
  });
});

describe("plain", () => {
  it("drops case and accents", () => {
    expect(plain("Café CRÈME")).toBe("cafe creme");
  });
});

describe("matchingExpenses", () => {
  it("keeps everything without a filter", () => {
    expect(ids({})).toEqual(["taxi", "cafe", "payment", "refund"]);
  });

  it("keeps a kind", () => {
    expect(ids({ kind: "expenses" })).toEqual(["taxi", "cafe"]);
    expect(ids({ kind: "income" })).toEqual(["refund"]);
    expect(ids({ kind: "payments" })).toEqual(["payment"]);
  });

  it("keeps what someone paid or owes part of", () => {
    expect(ids({ person: "carol" })).toEqual(["cafe"]);
    expect(ids({ person: "bob" })).toEqual(["taxi", "cafe", "payment", "refund"]);
  });

  it("keeps a category, or the expenses without one", () => {
    expect(ids({ category: "food" })).toEqual(["cafe"]);
    expect(ids({ category: "" })).toEqual(["taxi", "refund"]);
  });

  it("searches the title without minding accents or case", () => {
    expect(ids({ query: "CAFE" })).toEqual(["cafe"]);
  });

  it("searches the category, who paid and the amount", () => {
    expect(ids({ query: "restaurants" })).toEqual(["cafe"]);
    expect(ids({ query: "bob" })).toEqual(["cafe", "payment"]);
    expect(ids({ query: "8.50" })).toEqual(["cafe"]);
    expect(ids({ query: "€30" })).toEqual(["taxi", "refund"]);
  });

  it("needs every word of the search", () => {
    expect(ids({ query: "bob cafe" })).toEqual(["cafe"]);
    expect(ids({ query: "bob taxi" })).toEqual([]);
  });

  it("combines the filters", () => {
    expect(ids({ kind: "expenses", person: "bob", query: "taxi" })).toEqual(["taxi"]);
  });
});

describe("sortedExpenses", () => {
  const order = (sort: Parameters<typeof sortedExpenses>[1]) =>
    sortedExpenses(trip.expenses, sort).map((e) => e.id);

  it("orders by date", () => {
    expect(order("newest")).toEqual(["cafe", "refund", "payment", "taxi"]);
    expect(order("oldest")).toEqual(["taxi", "payment", "refund", "cafe"]);
  });

  it("orders by amount, the latest first among equals", () => {
    expect(order("highest")).toEqual(["refund", "taxi", "payment", "cafe"]);
    expect(order("lowest")).toEqual(["cafe", "payment", "refund", "taxi"]);
  });

  it("orders by title", () => {
    expect(order("title")).toEqual(["cafe", "refund", "payment", "taxi"]);
  });

  it("leaves the group's own list as it was", () => {
    order("oldest");
    expect(trip.expenses.map((e) => e.id)).toEqual(["taxi", "cafe", "payment", "refund"]);
  });
});

describe("spendingCents", () => {
  it("counts neither payments nor money that came in", () => {
    expect(spendingCents(trip.expenses)).toBe(3850);
  });
});

describe("dateGroups", () => {
  it("puts each day's expenses together, with what the day spent", () => {
    vi.useFakeTimers({ now: new Date(2026, 2, 12, 18), toFake: ["Date"] });
    const days = dateGroups(sortedExpenses(trip.expenses, "newest"), true);
    expect(days.map((d) => [d.displayDate, d.items.map((e) => e.id), d.totalCents])).toEqual([
      ["Today", ["cafe", "refund"], 850],
      ["Yesterday", ["payment"], 0],
      ["Tue, Mar 10", ["taxi"], 3000],
    ]);
    expect(days[0].dateKey).toBe("2026-03-12");
  });

  it("makes one list when the order isn't by date", () => {
    const [all, ...others] = dateGroups(trip.expenses, false);
    expect(others).toHaveLength(0);
    expect(all.dateKey).toBe("all");
    expect(all.items).toHaveLength(4);
    expect(all.totalCents).toBe(3850);
  });
});

describe("isForEveryone", () => {
  it("is an expense every member shares equally", () => {
    expect(isForEveryone(trip, expense({ splits: equally("alice", "bob", "carol") }))).toBe(true);
    expect(isForEveryone(trip, taxi)).toBe(false);
  });

  it("isn't when someone has more parts or an amount of their own", () => {
    const uneven = equally("alice", "bob", "carol");
    uneven[0] = { participant_id: "alice", shares: 2 };
    expect(isForEveryone(trip, expense({ splits: uneven }))).toBe(false);
    const fixed = equally("alice", "bob", "carol");
    fixed[2] = { participant_id: "carol", shares: 1, fixed_cents: 500 };
    expect(isForEveryone(trip, expense({ splits: fixed }))).toBe(false);
  });

  it("doesn't count the members who left", () => {
    const smaller = group({
      participants: [
        participant("alice"),
        participant("bob"),
        participant("carol", { removed: true }),
      ],
    });
    expect(isForEveryone(smaller, expense({ splits: equally("alice", "bob") }))).toBe(true);
    expect(isForEveryone(smaller, expense({ splits: equally("alice", "carol") }))).toBe(false);
  });
});

describe("usedCategories", () => {
  it("offers the categories the group uses, then the expenses without one", () => {
    expect(usedCategories(trip)).toEqual([
      { value: "food", label: "Restaurants" },
      { value: "", label: "No category" },
    ]);
  });

  it("offers nothing for a group of payments only", () => {
    expect(usedCategories(group({ expenses: [payment] }))).toEqual([]);
  });
});
