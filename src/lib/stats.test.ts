import { afterEach, describe, expect, it } from "vitest";
import { equally, expense, group, participant } from "@/test/fixtures";
import { i18n } from "./i18n/index.svelte";
import { byCategory, byMonth, byPerson, incomeCents, spending } from "./stats";

const at = (month: number, day: number) => new Date(2026, month - 1, day, 12).toISOString();

const trip = group({
  expenses: [
    expense({
      id: "dinner",
      category: "food",
      amount_cents: 3000,
      paid_by: "alice",
      splits: equally("alice", "bob"),
      created_at: at(3, 10),
    }),
    expense({
      id: "lunch",
      category: "food",
      amount_cents: 1000,
      paid_by: "bob",
      splits: equally("bob"),
      created_at: at(3, 12),
    }),
    expense({
      id: "taxi",
      category: "transport",
      amount_cents: 5000,
      paid_by: "alice",
      splits: equally("alice", "bob", "carol"),
      created_at: at(2, 1),
    }),
    expense({ id: "misc", amount_cents: 500, splits: equally("carol"), created_at: at(2, 2) }),
    expense({
      id: "payment",
      amount_cents: 1500,
      paid_by: "bob",
      splits: equally("alice"),
      is_reimbursement: true,
    }),
    expense({ id: "refund", amount_cents: 700, income: true }),
  ],
});

afterEach(() => i18n.choose("en"));

describe("spending", () => {
  it("leaves out payments and money that came in, which is counted apart", () => {
    expect(spending(trip).map((e) => e.id)).toEqual(["dinner", "lunch", "taxi", "misc"]);
    expect(incomeCents(trip)).toBe(700);
  });
});

describe("byCategory", () => {
  it("totals each category, the biggest first, with how many expenses", () => {
    expect(byCategory(spending(trip))).toEqual([
      { key: "transport", cents: 5000, count: 1 },
      { key: "food", cents: 4000, count: 2 },
      { key: "", cents: 500, count: 1 },
    ]);
  });

  it("files a category this version doesn't know under Other", () => {
    expect(byCategory([expense({ category: "spaceships" })])[0].key).toBe("other");
  });

  it("is empty without spending", () => {
    expect(byCategory([])).toEqual([]);
  });
});

describe("byPerson", () => {
  it("gives each person their share and what they paid, the biggest share first", () => {
    const people = byPerson(trip, spending(trip)).map((p) => [p.id, p.share, p.paid]);
    expect(people).toEqual([
      ["bob", 1500 + 1000 + 1667, 1000],
      ["alice", 1500 + 1667, 8500],
      ["carol", 1666 + 500, 0],
    ]);
  });

  it("lists someone who left only while they have a part in the spending", () => {
    const smaller = group({
      participants: [
        participant("alice"),
        participant("bob", { removed: true }),
        participant("carol", { removed: true }),
      ],
      expenses: [expense({ splits: equally("alice", "bob") })],
    });
    expect(byPerson(smaller, spending(smaller)).map((p) => p.id)).toEqual(["alice", "bob"]);
  });

  it("ignores a share of someone the group doesn't list", () => {
    const odd = group({ expenses: [expense({ splits: equally("alice", "ghost") })] });
    expect(byPerson(odd, spending(odd)).map((p) => p.id)).toEqual(["alice", "bob", "carol"]);
  });
});

describe("byMonth", () => {
  it("totals each calendar month, the latest first, with bars scaled to the biggest", () => {
    expect(byMonth(spending(trip))).toEqual([
      { key: "2026-03", name: "March 2026", cents: 4000, width: (4000 / 5500) * 100 },
      { key: "2026-02", name: "February 2026", cents: 5500, width: 100 },
    ]);
  });

  it("names the months in the app's language", () => {
    i18n.choose("fr");
    expect(byMonth(spending(trip)).map((m) => m.name)).toEqual(["mars 2026", "février 2026"]);
  });
});
