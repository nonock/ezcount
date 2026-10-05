import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "@/lib/i18n/index.svelte";
import { dialogs } from "@/lib/state/dialogs.svelte";
import { openGroup } from "@/lib/state/groups.svelte";
import { session } from "@/lib/state/session.svelte";
import { api } from "@/services/api";
import { account, equally, expense, group } from "@/test/fixtures";
import AddExpenseModal from "./AddExpenseModal.svelte";

vi.mock("@/services/api", () => ({
  api: { addExpense: vi.fn(), updateExpense: vi.fn(), suggestExchangeRate: vi.fn() },
}));
vi.mock("@/services/native.svelte", () => ({ closeTopLayer: () => false }));

async function type(field: HTMLElement, value: string) {
  await fireEvent.input(field, { target: { value } });
}

async function submit() {
  const form = (await screen.findByRole("dialog")).querySelector("form");
  if (!form) throw new Error("The dialog has no form");
  await fireEvent.submit(form);
}

beforeEach(() => {
  vi.clearAllMocks();
  session.account = account({ identities: { g1: "bob" } });
  openGroup.group = group();
  dialogs.expense = { open: false, editing: null };
  vi.spyOn(openGroup, "change").mockImplementation(async (apply) => {
    await apply("g1");
  });
});

describe("AddExpenseModal", () => {
  it("adds the expense typed, paid by the user and split between everyone", async () => {
    dialogs.openExpense();
    render(AddExpenseModal, { group: group() });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("expense.addTitle"));
    await type(screen.getByLabelText(t("expense.description")), "Taxi");
    await type(screen.getByLabelText(t("common.amount")), "30");
    await submit();
    expect(api.addExpense).toHaveBeenCalledTimes(1);
    const [groupId, input] = vi.mocked(api.addExpense).mock.calls[0];
    expect(groupId).toBe("g1");
    expect(input).toMatchObject({
      title: "Taxi",
      amount_cents: 3000,
      paid_by: "bob",
      income: false,
    });
    expect(input.splits).toEqual([
      { participant_id: "alice", shares: 1, fixed_cents: null },
      { participant_id: "bob", shares: 1, fixed_cents: null },
      { participant_id: "carol", shares: 1, fixed_cents: null },
    ]);
    await vi.waitFor(() => expect(dialogs.expense.open).toBe(false));
  });

  it("says what is missing instead of saving", async () => {
    dialogs.openExpense();
    render(AddExpenseModal, { group: group() });
    await type(await screen.findByLabelText(t("expense.description")), "Taxi");
    await submit();
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("expense.needAmount"));
    expect(api.addExpense).not.toHaveBeenCalled();
    expect(dialogs.expense.open).toBe(true);
  });

  it("opens an expense as it is, and saves its changes", async () => {
    const taxi = expense({ title: "Taxi", amount_cents: 3000, splits: equally("alice", "bob") });
    dialogs.openExpense(taxi);
    render(AddExpenseModal, { group: group({ expenses: [taxi] }) });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("expense.editTitle"));
    const title = screen.getByLabelText(t("expense.description")) as HTMLInputElement;
    expect(title.value).toBe("Taxi");
    expect((screen.getByLabelText(t("common.amount")) as HTMLInputElement).value).toBe("30");
    await type(title, "Cab");
    await submit();
    const [groupId, expenseId, input] = vi.mocked(api.updateExpense).mock.calls[0];
    expect([groupId, expenseId]).toEqual(["g1", "e1"]);
    expect(input).toMatchObject({ title: "Cab", amount_cents: 3000, paid_by: "alice" });
    expect(input.splits.map((s) => s.participant_id)).toEqual(["alice", "bob"]);
  });

  it("stays open and says why the core refused the expense", async () => {
    vi.mocked(api.addExpense).mockRejectedValue(new Error("Amount is too large"));
    dialogs.openExpense();
    render(AddExpenseModal, { group: group() });
    await type(await screen.findByLabelText(t("expense.description")), "Yacht");
    await type(screen.getByLabelText(t("common.amount")), "30");
    await submit();
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() => expect(dialog.textContent).toContain("Amount is too large"));
    expect(dialogs.expense.open).toBe(true);
  });

  it("starts empty again at each opening", async () => {
    dialogs.openExpense();
    const { rerender } = render(AddExpenseModal, { group: group() });
    await type(await screen.findByLabelText(t("expense.description")), "Taxi");
    dialogs.expense.open = false;
    await rerender({ group: group() });
    dialogs.openExpense();
    await rerender({ group: group() });
    const title = (await screen.findByLabelText(t("expense.description"))) as HTMLInputElement;
    expect(title.value).toBe("");
  });
});
