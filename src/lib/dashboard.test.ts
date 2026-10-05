import { describe, expect, it } from "vitest";
import { equally, expense, group } from "@/test/fixtures";
import { groupNets, totalsByCurrency } from "./dashboard";

// Alice paid 30 for her and Bob: the group owes her 15, Bob owes it 15.
const trip = group({ id: "trip", expenses: [expense({ splits: equally("alice", "bob") })] });
// Bob paid 20 for Alice alone.
const flat = group({
  id: "flat",
  expenses: [expense({ amount_cents: 2000, paid_by: "bob", splits: equally("alice") })],
});
const abroad = group({
  id: "abroad",
  currency: "USD",
  expenses: [expense({ amount_cents: 4000, splits: equally("bob") })],
});

const asAlice = () => "alice";

describe("groupNets", () => {
  it("is what each group owes the user, or the user owes it", () => {
    expect([...groupNets([trip, flat], asAlice)]).toEqual([
      ["trip", 1500],
      ["flat", -2000],
    ]);
  });

  it("leaves out the groups where the user didn't say who they are", () => {
    const nets = groupNets([trip, flat], (id) => (id === "trip" ? "bob" : null));
    expect([...nets]).toEqual([["trip", -1500]]);
  });
});

describe("totalsByCurrency", () => {
  it("adds what is owed to the user and what they owe, per currency", () => {
    const groups = [trip, flat, abroad];
    expect(totalsByCurrency(groups, groupNets(groups, asAlice))).toEqual([
      { currency: "EUR", owed: 1500, owes: 2000 },
      { currency: "USD", owed: 4000, owes: 0 },
    ]);
  });

  it("only counts the groups given, and none that is settled", () => {
    const nets = groupNets([trip, flat], asAlice);
    expect(totalsByCurrency([trip], nets)).toEqual([{ currency: "EUR", owed: 1500, owes: 0 }]);
    expect(totalsByCurrency([group({ id: "new" })], nets)).toEqual([]);
  });
});
