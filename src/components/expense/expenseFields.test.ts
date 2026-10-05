import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ExpenseForm, SEVERAL } from "@/lib/expenseForm.svelte";
import { group } from "@/test/fixtures";
import ExpenseAmountField from "./ExpenseAmountField.svelte";
import ExpenseKindTabs from "./ExpenseKindTabs.svelte";
import ExpensePayersField from "./ExpensePayersField.svelte";
import ExpenseRateField from "./ExpenseRateField.svelte";
import ExpenseSplitField from "./ExpenseSplitField.svelte";

vi.mock("@/services/api", () => ({ api: { suggestExchangeRate: vi.fn() } }));

let form: ExpenseForm;

beforeEach(() => {
  const trip = group();
  form = new ExpenseForm({ group: () => trip, editing: () => null, me: () => null });
  form.reset();
});

/** Types in a number field, as Svelte hears it. */
async function type(input: HTMLElement, value: string) {
  await fireEvent.input(input, { target: { value } });
}

describe("ExpenseKindTabs", () => {
  it("shows what is being added and tells when another kind is picked", async () => {
    const onChange = vi.fn();
    render(ExpenseKindTabs, { kind: "expense", onChange });
    expect(screen.getByRole("tab", { name: "Expense" }).getAttribute("aria-selected")).toBe("true");
    await fireEvent.click(screen.getByRole("tab", { name: "Transfer" }));
    expect(onChange).toHaveBeenCalledWith("transfer");
  });
});

describe("ExpenseAmountField", () => {
  it("takes the amount typed", async () => {
    render(ExpenseAmountField, { form });
    await type(screen.getByLabelText("Amount"), "12.5");
    expect(form.paidCents).toBe(1250);
  });

  it("is read-only once the payers or the lines give the amount", () => {
    form.choosePayer(SEVERAL);
    render(ExpenseAmountField, { form });
    const amount = screen.getByLabelText("Amount");
    expect(amount.hasAttribute("readonly")).toBe(true);
    expect(amount.getAttribute("aria-describedby")).toBe("expense-amount-is-total");
  });

  it("shows the currency by its sign", () => {
    render(ExpenseAmountField, { form });
    expect(screen.getByLabelText("Currency").textContent).toContain("€");
  });
});

describe("ExpenseRateField", () => {
  beforeEach(() => {
    form.currency = "USD";
    form.amountStr = "100";
  });

  it("says what the expense counts as in the group's currency", async () => {
    const { container } = render(ExpenseRateField, { form });
    expect(container.textContent).toContain("1 USD =");
    await type(screen.getByLabelText("Exchange rate"), "0.9");
    expect(form.rateIsOwn).toBe(true);
    expect(container.textContent).toContain("€90");
  });

  it("marks a rate that was only suggested", () => {
    form.rateStr = "0.9";
    const suggested = render(ExpenseRateField, { form }).container.textContent;
    form.rateIsOwn = true;
    const own = render(ExpenseRateField, { form }).container.textContent;
    expect(suggested?.length).toBeGreaterThan(own?.length ?? 0);
  });
});

describe("ExpensePayersField", () => {
  it("asks an amount of each person ticked, and totals them", async () => {
    form.choosePayer(SEVERAL);
    render(ExpensePayersField, { form });
    expect(screen.queryByLabelText("Amount paid by Bob")).toBeNull();
    await fireEvent.click(screen.getByRole("checkbox", { name: "Bob paid part of it" }));
    await type(screen.getByLabelText("Amount paid by Bob"), "7.5");
    // Alice, who was paying alone, is the first of them.
    expect(form.payers).toEqual([
      { participant_id: "alice", amount_cents: 0 },
      { participant_id: "bob", amount_cents: 750 },
    ]);
    expect(form.amountStr).toBe("7.50");
  });
});

describe("ExpenseSplitField", () => {
  it("counts the people and the parts", () => {
    form.amountStr = "30";
    const { container } = render(ExpenseSplitField, { form });
    expect(container.textContent).toContain("3/3 people");
    expect(container.textContent).toContain("about €10 per part");
  });

  it("gives someone more parts, and never fewer than one", async () => {
    render(ExpenseSplitField, { form });
    expect(screen.getByRole("button", { name: "Fewer parts for Alice" })).toHaveProperty(
      "disabled",
      true
    );
    await fireEvent.click(screen.getByRole("button", { name: "More parts for Alice" }));
    expect(form.splitsState.alice.shares).toBe(2);
  });

  it("leaves someone out", async () => {
    render(ExpenseSplitField, { form });
    await fireEvent.click(screen.getByRole("checkbox", { name: "Carol" }));
    expect(form.includedParticipants.map((p) => p.id)).toEqual(["alice", "bob"]);
    expect(screen.queryByRole("button", { name: "More parts for Carol" })).toBeNull();
  });

  it("sets an amount for someone instead of parts, and says when it doesn't fit", async () => {
    form.amountStr = "30";
    const { container } = render(ExpenseSplitField, { form });
    await fireEvent.click(screen.getByRole("button", { name: "Set an amount for Bob" }));
    await type(screen.getByLabelText("Amount for Bob"), "40");
    expect(form.splits[1]).toEqual({ participant_id: "bob", shares: 0, fixed_cents: 4000 });
    expect(container.querySelector(".text-destructive")?.textContent).toContain("€10");
  });

  it("ticks or unticks everyone at once", async () => {
    render(ExpenseSplitField, { form });
    await fireEvent.click(screen.getByRole("button", { name: "Deselect all" }));
    expect(form.includedParticipants).toHaveLength(0);
    await fireEvent.click(screen.getByRole("button", { name: "Select all" }));
    expect(form.allIncluded).toBe(true);
  });
});
