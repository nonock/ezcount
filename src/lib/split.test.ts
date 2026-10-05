import { describe, expect, it } from "vitest";
import { equally, expense, group } from "@/test/fixtures";
import {
  isSpending,
  itemsOwed,
  netBalance,
  owedAmounts,
  paidAmounts,
  paidCurrency,
  spendingOf,
  spentCents,
} from "./split";

describe("owedAmounts", () => {
  it("divides by parts, the leftover cents going to the largest remainders", () => {
    expect(owedAmounts(1000, null, equally("a", "b", "c"))).toEqual([334, 333, 333]);
    expect(
      owedAmounts(1000, null, [
        { participant_id: "a", shares: 2 },
        { participant_id: "b", shares: 1 },
      ])
    ).toEqual([667, 333]);
  });

  it("gives nothing when no one has a part", () => {
    expect(owedAmounts(1000, null, [{ participant_id: "a", shares: 0 }])).toEqual([0]);
    expect(owedAmounts(1000, null, [])).toEqual([]);
  });

  it("takes the fixed amounts first and shares the rest", () => {
    expect(
      owedAmounts(1000, null, [
        { participant_id: "a", shares: 0, fixed_cents: 400 },
        { participant_id: "b", shares: 1 },
        { participant_id: "c", shares: 2 },
      ])
    ).toEqual([400, 200, 400]);
  });

  it("leaves nothing to share when the fixed amounts exceed the expense", () => {
    expect(
      owedAmounts(1000, null, [
        { participant_id: "a", shares: 0, fixed_cents: 1200 },
        { participant_id: "b", shares: 1 },
      ])
    ).toEqual([1200, 0]);
  });

  it("converts fixed amounts typed in the currency paid", () => {
    // 100.00 paid abroad is 90.00 here: 40 fixed and 60 shared there, 36 and 54 here.
    expect(
      owedAmounts(9000, 10000, [
        { participant_id: "a", shares: 0, fixed_cents: 4000 },
        { participant_id: "b", shares: 1 },
      ])
    ).toEqual([3600, 5400]);
  });

  it("ignores the amount paid abroad when there are only parts", () => {
    expect(owedAmounts(9000, 10000, equally("a", "b"))).toEqual([4500, 4500]);
  });
});

describe("paidAmounts", () => {
  it("credits the only payer the whole amount", () => {
    expect(paidAmounts(expense({ amount_cents: 3000, paid_by: "alice" }))).toEqual([
      { id: "alice", cents: 3000 },
    ]);
  });

  it("credits several payers what each paid", () => {
    const paid = paidAmounts(
      expense({
        amount_cents: 5000,
        payers: [
          { participant_id: "alice", amount_cents: 3000 },
          { participant_id: "bob", amount_cents: 2000 },
        ],
      })
    );
    expect(paid).toEqual([
      { id: "alice", cents: 3000 },
      { id: "bob", cents: 2000 },
    ]);
  });

  it("converts what several payers paid in another currency", () => {
    const paid = paidAmounts(
      expense({
        amount_cents: 9000,
        original: { currency: "USD", amount_cents: 10000, rate: "0.9" },
        payers: [
          { participant_id: "alice", amount_cents: 7500 },
          { participant_id: "bob", amount_cents: 2500 },
        ],
      })
    );
    expect(paid).toEqual([
      { id: "alice", cents: 6750 },
      { id: "bob", cents: 2250 },
    ]);
  });
});

describe("itemsOwed", () => {
  it("shares each line equally, the cents left over going to the first people", () => {
    const owed = itemsOwed([
      { amount_cents: 1000, participants: ["alice", "bob", "carol"] },
      { amount_cents: 500, participants: ["bob"] },
    ]);
    expect([...owed]).toEqual([
      ["alice", 334],
      ["bob", 833],
      ["carol", 333],
    ]);
  });

  it("skips a line no one shares", () => {
    expect(itemsOwed([{ amount_cents: 1000, participants: [] }]).size).toBe(0);
  });
});

describe("spending", () => {
  const trip = group({
    expenses: [
      expense({ id: "e1", amount_cents: 3000, paid_by: "alice", splits: equally("alice", "bob") }),
      expense({ id: "e2", amount_cents: 1000, paid_by: "bob", splits: equally("carol") }),
      expense({
        id: "pay",
        amount_cents: 1500,
        paid_by: "bob",
        splits: equally("alice"),
        is_reimbursement: true,
      }),
      expense({
        id: "refund",
        amount_cents: 600,
        paid_by: "alice",
        splits: equally("alice", "bob", "carol"),
        income: true,
      }),
    ],
  });

  it("counts neither payments nor money that came in", () => {
    expect(isSpending(trip.expenses[0])).toBe(true);
    expect(isSpending(trip.expenses[2])).toBe(false);
    expect(isSpending(trip.expenses[3])).toBe(false);
    expect(spentCents(trip)).toBe(4000);
  });

  it("gives a member's share of the spending and what they paid of it", () => {
    expect(spendingOf(trip, "alice")).toEqual({ share: 1500, paid: 3000 });
    expect(spendingOf(trip, "bob")).toEqual({ share: 1500, paid: 1000 });
    expect(spendingOf(trip, "carol")).toEqual({ share: 1000, paid: 0 });
  });

  it("balances payments and counts income the other way", () => {
    // Alice: +3000 -1500 (taxi), -1500 (paid back by Bob), -600 +200 (holds the refund).
    expect(netBalance(trip, "alice")).toBe(-400);
    expect(netBalance(trip, "bob")).toBe(1200);
    expect(netBalance(trip, "carol")).toBe(-800);
  });

  it("leaves the balances adding up to zero", () => {
    const total = trip.participants.reduce((sum, p) => sum + netBalance(trip, p.id), 0);
    expect(total).toBe(0);
  });
});

describe("paidCurrency", () => {
  it("is the currency paid in, the group's otherwise", () => {
    expect(paidCurrency(expense(), "EUR")).toBe("EUR");
    const abroad = expense({ original: { currency: "USD", amount_cents: 100, rate: "0.9" } });
    expect(paidCurrency(abroad, "EUR")).toBe("USD");
  });
});
