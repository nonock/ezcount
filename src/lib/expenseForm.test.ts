import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { equally, expense, group, participant } from "@/test/fixtures";
import type { Expense, ExpenseInput, Group } from "@/types";
import { atThisTime, ExpenseForm, SEVERAL, toCents } from "./expenseForm.svelte";
import { t } from "./i18n/index.svelte";

vi.mock("@/services/api", () => ({ api: { suggestExchangeRate: vi.fn() } }));

let trip: Group;
let editing: Expense | null;
let me: string | null;

/** A form on `trip`, as the dialog opens it. */
function open(): ExpenseForm {
  const form = new ExpenseForm({ group: () => trip, editing: () => editing, me: () => me });
  form.reset();
  return form;
}

/** The expense a form that should be valid makes. */
function inputOf(form: ExpenseForm): ExpenseInput {
  const result = form.result();
  if (result.error !== undefined) throw new Error(result.error);
  return result.input;
}

beforeEach(() => {
  trip = group();
  editing = null;
  me = null;
  vi.mocked(api.suggestExchangeRate).mockReset();
});

describe("toCents", () => {
  it("reads an amount typed, as text or as the number an input binds", () => {
    expect(toCents("12.5")).toBe(1250);
    expect(toCents(12.5)).toBe(1250);
    expect(toCents("19.99")).toBe(1999);
    expect(toCents("0.07")).toBe(7);
  });

  it("counts what isn't a number as nothing", () => {
    expect(toCents("")).toBe(0);
    expect(toCents(null)).toBe(0);
    expect(toCents("abc")).toBe(0);
  });
});

describe("atThisTime", () => {
  it("is the day picked, here", () => {
    const at = new Date(atThisTime("2026-03-15") ?? "");
    expect([at.getFullYear(), at.getMonth(), at.getDate()]).toEqual([2026, 2, 15]);
  });

  it("is nothing without a day", () => {
    expect(atThisTime("")).toBeNull();
  });
});

describe("a new expense", () => {
  it("starts empty, split equally between everyone", () => {
    const form = open();
    expect(form.title).toBe("");
    expect(form.currency).toBe("EUR");
    expect(form.paidBy).toBe("alice");
    expect(form.allIncluded).toBe(true);
    expect(form.splits).toEqual([
      { participant_id: "alice", shares: 1, fixed_cents: null },
      { participant_id: "bob", shares: 1, fixed_cents: null },
      { participant_id: "carol", shares: 1, fixed_cents: null },
    ]);
  });

  it("is paid by the user when they said who they are", () => {
    me = "bob";
    expect(open().paidBy).toBe("bob");
    me = "someone who left";
    expect(open().paidBy).toBe("alice");
  });

  it("leaves out the members who were removed", () => {
    trip = group({ participants: [participant("alice"), participant("bob", { removed: true })] });
    expect(open().participants.map((p) => p.id)).toEqual(["alice"]);
  });

  it("offers the group's currency first", () => {
    trip = group({ currency: "CHF" });
    expect(open().currencies[0]).toBe("CHF");
    expect(new Set(open().currencies).size).toBe(open().currencies.length);
  });
});

describe("an expense being edited", () => {
  it("fills the form with it", () => {
    editing = expense({
      title: "Hotel",
      category: "housing",
      amount_cents: 9000,
      paid_by: "bob",
      created_at: new Date(2026, 2, 15, 12).toISOString(),
      splits: [
        { participant_id: "alice", shares: 0, fixed_cents: 4000 },
        { participant_id: "bob", shares: 2 },
      ],
    });
    const form = open();
    expect(form.title).toBe("Hotel");
    expect(form.category).toBe("housing");
    expect(form.amountStr).toBe("90");
    expect(form.paidBy).toBe("bob");
    expect(form.expenseDate).toBe("2026-03-15");
    expect(form.splitsState.alice).toEqual({
      included: true,
      shares: 1,
      fixed: true,
      amount: "40",
    });
    expect(form.splitsState.bob).toMatchObject({ included: true, shares: 2, fixed: false });
    expect(form.splitsState.carol.included).toBe(false);
  });

  it("shows what was paid in another currency, with its rate as the user's own", () => {
    editing = expense({
      amount_cents: 9000,
      original: { currency: "JPY", amount_cents: 10000, rate: "0.9" },
    });
    const form = open();
    expect(form.currency).toBe("JPY");
    expect(form.currencies).toContain("JPY");
    expect(form.amountStr).toBe("100");
    expect(form.rateStr).toBe("0.9");
    expect(form.rateIsOwn).toBe(true);
    expect(form.amountCents).toBe(9000);
  });

  it("keeps a removed member who is part of it", () => {
    trip = group({
      participants: [
        participant("alice"),
        participant("bob", { removed: true }),
        participant("carol", { removed: true }),
      ],
    });
    editing = expense({ paid_by: "alice", splits: equally("alice", "bob") });
    expect(open().participants.map((p) => p.id)).toEqual(["alice", "bob"]);
  });

  it("shows its payers and its lines", () => {
    editing = expense({
      amount_cents: 5000,
      payers: [
        { participant_id: "alice", amount_cents: 3000 },
        { participant_id: "bob", amount_cents: 2000 },
      ],
      items: [{ name: "Wine", amount_cents: 5000, participants: ["alice", "bob"] }],
    });
    const form = open();
    expect(form.severalPayers).toBe(true);
    expect(form.payersState.alice).toEqual({ included: true, amount: "30" });
    expect(form.payersState.carol.included).toBe(false);
    expect(form.byItems).toBe(true);
    expect(form.itemsState[0]).toMatchObject({ name: "Wine", amount: "50" });
  });
});

describe("the split", () => {
  it("shows what each person owes as it is typed", () => {
    const form = open();
    form.amountStr = 10;
    expect([...form.owed.values()]).toEqual([334, 333, 333]);
    form.updateShares("alice", 2);
    expect(form.totalShares).toBe(4);
    expect(form.owed.get("alice")).toBe(500);
  });

  it("never gives someone less than one part", () => {
    const form = open();
    form.updateShares("alice", 0);
    expect(form.splitsState.alice.shares).toBe(1);
  });

  it("ticks everyone, or no one once everyone is", () => {
    const form = open();
    form.toggleAll();
    expect(form.includedParticipants).toHaveLength(0);
    form.toggleParticipant("bob");
    expect(form.includedParticipants.map((p) => p.id)).toEqual(["bob"]);
    form.toggleAll();
    expect(form.allIncluded).toBe(true);
  });

  it("takes in someone who joined while the form is open", () => {
    const form = open();
    trip = group({ participants: [...trip.participants, participant("dan")] });
    expect(form.stateOf("dan")).toEqual({ included: false, shares: 1, fixed: false, amount: "" });
  });

  it("says when the set amounts are too much, or leave money to no one", () => {
    const form = open();
    form.amountStr = "10";
    for (const id of ["alice", "bob", "carol"]) form.splitsState[id].fixed = true;
    form.splitsState.alice.amount = "8";
    form.splitsState.bob.amount = "4";
    expect(form.splitProblem).toContain("€2");
    form.splitsState.bob.amount = "1";
    expect(form.restCents).toBe(100);
    expect(form.splitProblem).toContain("€1");
    form.splitsState.carol.amount = "1";
    expect(form.splitProblem).toBeNull();
  });
});

describe("several payers", () => {
  it("start from who was paying alone, with the whole amount", () => {
    const form = open();
    form.amountStr = "30";
    form.choosePayer(SEVERAL);
    expect(form.severalPayers).toBe(true);
    expect(form.payersState.alice).toEqual({ included: true, amount: "30" });
  });

  it("give the expense its amount", () => {
    const form = open();
    form.choosePayer(SEVERAL);
    form.togglePayer("bob");
    form.payersState.alice.amount = "12.50";
    form.payersState.bob.amount = "7.5";
    form.totalPayers();
    expect(form.amountStr).toBe("20");
    form.togglePayer("bob");
    expect(form.amountStr).toBe("12.50");
  });

  it("go back to one", () => {
    const form = open();
    form.choosePayer(SEVERAL);
    form.choosePayer("carol");
    expect(form.severalPayers).toBe(false);
    expect(form.paidBy).toBe("carol");
  });
});

describe("lines", () => {
  it("start with the whole expense, for everyone it was split between", () => {
    const form = open();
    form.amountStr = "30";
    form.toggleParticipant("carol");
    form.toggleItems();
    expect(form.byItems).toBe(true);
    expect(form.itemsState).toHaveLength(1);
    expect(form.itemsState[0]).toMatchObject({ amount: "30", people: ["alice", "bob"] });
  });

  it("give the expense its amount, unless several payers already do", () => {
    const form = open();
    form.toggleItems();
    form.itemsState[0].amount = "12";
    form.totalItems();
    expect(form.amountStr).toBe("12");
    expect([...form.itemsOwe]).toEqual([
      ["alice", 400],
      ["bob", 400],
      ["carol", 400],
    ]);
    form.severalPayers = true;
    form.itemsState[0].amount = "20";
    form.totalItems();
    expect(form.amountStr).toBe("12");
  });

  it("are kept when going back to a split between people", () => {
    const form = open();
    form.toggleItems();
    form.toggleItems();
    expect(form.byItems).toBe(false);
    expect(form.itemsState).toHaveLength(1);
  });
});

describe("the expense the form makes", () => {
  const filled = () => {
    const form = open();
    form.title = "  Taxi ";
    form.amountStr = "30";
    form.expenseDate = "2026-03-15";
    return form;
  };

  it("is what was typed", () => {
    const input = inputOf(filled());
    expect(input).toMatchObject({
      title: "Taxi",
      category: null,
      amount_cents: 3000,
      paid_by: "alice",
      payers: [],
      items: [],
      original: null,
      income: false,
      repeat: null,
    });
    expect(input.splits).toHaveLength(3);
    expect(input.created_at).toContain("2026-03-1");
  });

  it("needs a description, an amount and someone to split it between", () => {
    const form = open();
    expect(form.result().error).toBe(t("expense.needDescription"));
    form.title = "Taxi";
    expect(form.result().error).toBe(t("expense.needAmount"));
    form.amountStr = "30";
    form.toggleAll();
    expect(form.result().error).toBe(t("expense.needPeople"));
  });

  it("needs a rate for another currency, one that leaves at least a cent", () => {
    const form = filled();
    form.currency = "USD";
    expect(form.result().error).toContain("USD");
    form.rateStr = "0,9";
    expect(inputOf(form)).toMatchObject({
      amount_cents: 2700,
      original: { currency: "USD", amount_cents: 3000, rate: "0,9" },
    });
    form.amountStr = "0.01";
    form.rateStr = "0.1";
    expect(form.result().error).toContain("EUR");
  });

  it("needs each payer's amount, and counts one ticked payer as paying alone", () => {
    const form = filled();
    form.choosePayer(SEVERAL);
    form.togglePayer("alice");
    expect(form.result().error).toBe(t("expense.needPayers"));
    form.togglePayer("bob");
    form.payersState.bob.amount = "30";
    form.totalPayers();
    expect(inputOf(form)).toMatchObject({ paid_by: "bob", payers: [] });
    form.togglePayer("carol");
    expect(form.result().error).toBe(t("expense.needPayerAmounts"));
    form.payersState.carol.amount = "10";
    form.totalPayers();
    expect(inputOf(form)).toMatchObject({
      amount_cents: 4000,
      paid_by: "bob",
      payers: [
        { participant_id: "bob", amount_cents: 3000 },
        { participant_id: "carol", amount_cents: 1000 },
      ],
    });
  });

  it("takes its splits from the lines", () => {
    const form = filled();
    form.toggleItems();
    form.itemsState[0].name = " Wine ";
    expect(inputOf(form)).toMatchObject({
      splits: [],
      items: [{ name: "Wine", amount_cents: 3000, participants: ["alice", "bob", "carol"] }],
    });
    form.itemsState[0].people = [];
    expect(form.result().error).toBe(t("expense.needItemPeople"));
    form.itemsState[0].amount = "";
    expect(form.result().error).toBe(t("expense.needItemAmounts"));
    form.itemsState = [];
    expect(form.result().error).toBe(t("expense.needItems"));
  });

  it("has the lines add up to what several payers paid", () => {
    const form = filled();
    form.toggleItems();
    form.severalPayers = true;
    form.payersState.alice = { included: true, amount: "30" };
    form.itemsState[0].amount = "25";
    expect(form.result().error).toBe(form.itemsMismatch);
  });

  it("needs set amounts above zero that fit the expense", () => {
    const form = filled();
    form.splitsState.alice.fixed = true;
    expect(form.result().error).toBe(t("expense.needSetAmounts"));
    form.splitsState.alice.amount = "40";
    expect(form.result().error).toBe(form.splitProblem);
    form.splitsState.alice.amount = "10";
    expect(inputOf(form).splits[0]).toEqual({
      participant_id: "alice",
      shares: 0,
      fixed_cents: 1000,
    });
  });

  it("repeats only a new expense in the group's currency", () => {
    const form = filled();
    form.repeat = "month";
    form.income = true;
    form.category = "food";
    expect(inputOf(form)).toMatchObject({ repeat: "month", income: true, category: "food" });
    form.currency = "USD";
    form.rateStr = "1";
    expect(inputOf(form).repeat).toBeNull();
    editing = expense();
    const edit = open();
    edit.repeat = "month";
    expect(inputOf(edit).repeat).toBeNull();
  });
});

describe("the exchange rate", () => {
  it("is suggested by the relay for the expense's day", async () => {
    vi.mocked(api.suggestExchangeRate).mockResolvedValue("0.92");
    const form = open();
    form.expenseDate = "2026-03-15";
    form.chooseCurrency("USD");
    expect(form.lookingUpRate).toBe(true);
    await vi.waitFor(() => expect(form.rateStr).toBe("0.92"));
    expect(api.suggestExchangeRate).toHaveBeenCalledWith("USD", "EUR", "2026-03-15");
    expect(form.lookingUpRate).toBe(false);
    expect(form.rateIsOwn).toBe(false);
  });

  it("falls back to the one last used in the group", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.mocked(api.suggestExchangeRate).mockRejectedValue(new Error("offline"));
    trip = group({
      expenses: [
        expense({ id: "e1", original: { currency: "USD", amount_cents: 100, rate: "0.8" } }),
        expense({ id: "e2", original: { currency: "USD", amount_cents: 100, rate: "0.85" } }),
      ],
    });
    const form = open();
    form.chooseCurrency("USD");
    await vi.waitFor(() => expect(form.rateStr).toBe("0.85"));
  });

  it("isn't asked for the group's currency, nor over a rate the user typed", async () => {
    const form = open();
    await form.suggestRate();
    form.currency = "USD";
    form.rateIsOwn = true;
    await form.suggestRate();
    expect(api.suggestExchangeRate).not.toHaveBeenCalled();
  });

  it("leaves a rate typed while it was looked up", async () => {
    let answer = (_rate: string) => {};
    vi.mocked(api.suggestExchangeRate).mockReturnValue(
      new Promise((resolve) => {
        answer = resolve;
      })
    );
    const form = open();
    form.chooseCurrency("USD");
    form.rateStr = "0.5";
    form.rateIsOwn = true;
    answer("0.92");
    await vi.waitFor(() => expect(form.lookingUpRate).toBe(false));
    expect(form.rateStr).toBe("0.5");
  });

  it("uses only the last lookup's answer", async () => {
    const answers: ((rate: string) => void)[] = [];
    vi.mocked(api.suggestExchangeRate).mockImplementation(
      () => new Promise((resolve) => answers.push(resolve))
    );
    const form = open();
    form.chooseCurrency("USD");
    form.chooseCurrency("GBP");
    answers[1]("1.17");
    await vi.waitFor(() => expect(form.rateStr).toBe("1.17"));
    answers[0]("0.92");
    await Promise.resolve();
    expect(form.rateStr).toBe("1.17");
  });

  it("follows the day picked", async () => {
    vi.mocked(api.suggestExchangeRate).mockResolvedValue("0.9");
    const form = open();
    form.currency = "USD";
    form.chooseDate("2026-01-02");
    await vi.waitFor(() => expect(form.rateStr).toBe("0.9"));
    expect(api.suggestExchangeRate).toHaveBeenCalledWith("USD", "EUR", "2026-01-02");
  });
});
