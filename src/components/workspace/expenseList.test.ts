import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { deleteExpense } from "@/lib/actions/expenses";
import { ExpenseFilter } from "@/lib/expenseList.svelte";
import { t } from "@/lib/i18n/index.svelte";
import { dialogs } from "@/lib/state/dialogs.svelte";
import { openGroup } from "@/lib/state/groups.svelte";
import { equally, expense, group } from "@/test/fixtures";
import ExpenseActions from "./ExpenseActions.svelte";
import ExpenseFilters from "./ExpenseFilters.svelte";
import ExpenseRow from "./ExpenseRow.svelte";

vi.mock("@/lib/actions/expenses", () => ({ deleteExpense: vi.fn() }));
vi.mock("@/services/api", () => ({ api: {} }));

const trip = group();

beforeEach(() => {
  dialogs.history = null;
  dialogs.comments = null;
  dialogs.expense = { open: false, editing: null };
  dialogs.reimburse.open = false;
  dialogs.trash = false;
  openGroup.settlements = [];
});

describe("ExpenseRow", () => {
  it("shows an expense with who paid and each part", () => {
    const { container } = render(ExpenseRow, { group: trip, expense: expense(), byDay: true });
    expect(screen.getByRole("heading", { name: "Taxi" })).toBeTruthy();
    expect(container.textContent).toContain(t("common.paidBy"));
    expect(container.textContent).toContain("€30");
    expect(container.textContent).toContain("€15");
    const sharing = screen.getByRole("list", { name: t("common.splitBetween") });
    const names = [...sharing.querySelectorAll("li")].map((li) => li.textContent?.trim());
    expect(names).toEqual(["Alice", "Bob"]);
  });

  it("says an expense is for everyone instead of listing them", () => {
    const forAll = expense({ splits: equally("alice", "bob", "carol") });
    const { container } = render(ExpenseRow, { group: trip, expense: forAll, byDay: true });
    expect(container.textContent).toContain(t("expenses.forEveryone"));
    expect(screen.queryByRole("list", { name: t("common.splitBetween") })).toBeNull();
  });

  it("shows parts and set amounts next to the names", () => {
    const uneven = expense({
      splits: [
        { participant_id: "alice", shares: 2 },
        { participant_id: "bob", shares: 0, fixed_cents: 500 },
      ],
    });
    render(ExpenseRow, { group: trip, expense: uneven, byDay: true });
    const sharing = screen.getByRole("list", { name: t("common.splitBetween") }).textContent;
    expect(sharing).toContain("Alice ×2");
    expect(sharing).toContain("Bob €5");
  });

  it("shows a payment from one member to another", () => {
    const payment = expense({
      title: "Payment: Bob → Alice",
      paid_by: "bob",
      splits: equally("alice"),
      is_reimbursement: true,
    });
    const { container } = render(ExpenseRow, { group: trip, expense: payment, byDay: true });
    expect(container.textContent).toContain(t("expenses.reimbursement"));
    expect(container.querySelector(".text-positive")).not.toBeNull();
    expect(screen.queryByRole("list", { name: t("common.splitBetween") })).toBeNull();
  });

  it("shows what was paid in another currency", () => {
    const abroad = expense({ original: { currency: "USD", amount_cents: 3300, rate: "0.9" } });
    const { container } = render(ExpenseRow, { group: trip, expense: abroad, byDay: true });
    expect(container.textContent).toContain("$33");
  });

  it("gives its date when the list isn't by day", () => {
    const old = expense({ created_at: new Date(2020, 0, 15, 12).toISOString() });
    const byDay = render(ExpenseRow, { group: trip, expense: old, byDay: true });
    expect(byDay.container.textContent).not.toContain("2020");
    const flat = render(ExpenseRow, { group: trip, expense: old, byDay: false });
    expect(flat.container.textContent).toContain("Jan 15, 2020");
  });

  it("opens the history and the comments from their badges", async () => {
    const busy = expense({
      history: [
        {
          edited_at: "2026-03-02T10:00:00Z",
          previous_title: "Cab",
          previous_amount_cents: 2000,
          previous_paid_by: "alice",
          previous_splits: [],
          summary: "Title changed from 'Cab' to 'Taxi'",
        },
      ],
      comments: [{ id: "c1", text: "Receipt?", created_at: "2026-03-03T10:00:00Z" }],
    });
    render(ExpenseRow, { group: trip, expense: busy, byDay: true });
    await fireEvent.click(
      screen.getByRole("button", { name: t("expenses.editedLabel", 1, "Taxi") })
    );
    expect(dialogs.history?.id).toBe("e1");
    await fireEvent.click(
      screen.getByRole("button", { name: t("comments.countLabel", 1, "Taxi") })
    );
    expect(dialogs.comments).toBe("e1");
  });

  it("has neither badge for an expense no one touched", () => {
    render(ExpenseRow, { group: trip, expense: expense(), byDay: true });
    expect(screen.queryByRole("button", { name: t("expenses.editedLabel", 1, "Taxi") })).toBeNull();
    expect(screen.queryByRole("button", { name: t("comments.countLabel", 1, "Taxi") })).toBeNull();
    expect(deleteExpense).not.toHaveBeenCalled();
  });
});

describe("ExpenseFilters", () => {
  const taxi = expense();

  it("searches as it is typed", async () => {
    const filter = new ExpenseFilter();
    render(ExpenseFilters, { group: trip, filter, matching: [taxi] });
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "taxi" } });
    expect(filter.query).toBe("taxi");
  });

  it("says nothing of the count until something is filtered", () => {
    const filter = new ExpenseFilter();
    const { container } = render(ExpenseFilters, { group: trip, filter, matching: [taxi] });
    expect(container.textContent).not.toContain(t("expenses.count", 1));
    expect(screen.queryByRole("button", { name: t("expenses.clear") })).toBeNull();
  });

  it("counts what the filter keeps and clears it", async () => {
    const filter = new ExpenseFilter();
    filter.person = "alice";
    const { container } = render(ExpenseFilters, { group: trip, filter, matching: [taxi] });
    expect(container.textContent).toContain(t("expenses.count", 1));
    expect(container.textContent).toContain("€30");
    expect(container.textContent).toContain(t("expenses.filtersOn"));
    await fireEvent.click(screen.getByRole("button", { name: t("expenses.clear") }));
    expect(filter.filtering).toBe(false);
  });

  it("marks the menu for an order other than the usual one", () => {
    const filter = new ExpenseFilter();
    filter.sort = "title";
    const { container } = render(ExpenseFilters, { group: trip, filter, matching: [taxi] });
    expect(container.textContent).toContain(t("expenses.filtersOn"));
  });
});

describe("ExpenseActions", () => {
  it("opens a new expense", async () => {
    render(ExpenseActions, { group: trip });
    await fireEvent.click(screen.getByRole("button", { name: t("expenses.add") }));
    expect(dialogs.expense).toEqual({ open: true, editing: null });
  });

  it("offers a payment only while someone owes something", async () => {
    const spent = group({ expenses: [expense()] });
    const settled = render(ExpenseActions, { group: spent });
    expect(screen.queryByRole("button", { name: t("expenses.reimburse") })).toBeNull();
    settled.unmount();

    openGroup.settlements = [
      { from_id: "bob", from_name: "Bob", to_id: "alice", to_name: "Alice", amount_cents: 1500 },
    ];
    render(ExpenseActions, { group: spent });
    await fireEvent.click(screen.getByRole("button", { name: t("expenses.reimburse") }));
    expect(dialogs.reimburse.open).toBe(true);
  });
});
