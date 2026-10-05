import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { purgeExpense, restoreExpense, stopRecurring } from "@/lib/actions/expenses";
import { t } from "@/lib/i18n/index.svelte";
import { dialogs } from "@/lib/state/dialogs.svelte";
import { expense, group } from "@/test/fixtures";
import type { RecurringExpense } from "@/types";
import RecurringDialog from "./RecurringDialog.svelte";
import TrashDialog from "./TrashDialog.svelte";

vi.mock("@/lib/actions/expenses", () => ({
  purgeExpense: vi.fn(),
  restoreExpense: vi.fn(),
  stopRecurring: vi.fn(),
}));

const rent: RecurringExpense = {
  id: "rent",
  title: "Rent",
  amount_cents: 90000,
  paid_by: "alice",
  splits: [],
  every: "month",
  next: new Date(2026, 3, 1, 12).toISOString(),
};

beforeEach(() => {
  vi.clearAllMocks();
  dialogs.trash = false;
  dialogs.recurring = false;
});

describe("TrashDialog", () => {
  const deleted = {
    expense: expense({ id: "gone", title: "Hotel", amount_cents: 12000 }),
    deleted_at: new Date(2026, 2, 6, 10, 30).toISOString(),
    deleted_by: "bob",
  };

  it("stays closed until asked for", () => {
    render(TrashDialog, { group: group({ trash: [deleted] }) });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("says so when nothing was deleted", async () => {
    dialogs.trash = true;
    render(TrashDialog, { group: group() });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("trash.empty"));
  });

  it("lists what was deleted, by whom, and puts it back or removes it for good", async () => {
    dialogs.trash = true;
    render(TrashDialog, { group: group({ trash: [deleted] }) });
    const list = await screen.findByRole("list", { name: t("trash.title") });
    expect(list.textContent).toContain("Hotel");
    expect(list.textContent).toContain("€120");
    expect(list.textContent).toContain("Bob");
    await fireEvent.click(
      within(list).getByRole("button", { name: t("trash.restoreLabel", "Hotel") })
    );
    expect(restoreExpense).toHaveBeenCalledWith("gone");
    await fireEvent.click(within(list).getByRole("button", { name: t("trash.purge", "Hotel") }));
    expect(purgeExpense).toHaveBeenCalledWith("gone");
  });

  it("doesn't name anyone when the group doesn't know who deleted", async () => {
    dialogs.trash = true;
    const unknown = { ...deleted, deleted_by: null };
    render(TrashDialog, { group: group({ trash: [unknown] }) });
    const list = await screen.findByRole("list", { name: t("trash.title") });
    expect(list.textContent).not.toContain("Bob");
  });
});

describe("RecurringDialog", () => {
  it("says so when nothing repeats", async () => {
    dialogs.recurring = true;
    render(RecurringDialog, { group: group() });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("recurring.empty"));
  });

  it("lists what repeats, how often and when next, and stops one", async () => {
    dialogs.recurring = true;
    render(RecurringDialog, { group: group({ recurring: [rent] }) });
    const list = await screen.findByRole("list", { name: t("recurring.title") });
    expect(list.textContent).toContain("Rent");
    expect(list.textContent).toContain("€900");
    expect(list.textContent).toContain(t("expense.repeatMonth"));
    expect(list.textContent).toContain("Apr 1, 2026");
    await fireEvent.click(
      within(list).getByRole("button", { name: t("recurring.stopLabel", "Rent") })
    );
    expect(stopRecurring).toHaveBeenCalledWith("rent");
  });

  it("says when one is paused because someone it names left", async () => {
    dialogs.recurring = true;
    const paused = { ...rent, every: "week", paused: true };
    render(RecurringDialog, { group: group({ recurring: [paused] }) });
    const list = await screen.findByRole("list", { name: t("recurring.title") });
    expect(list.textContent).toContain(t("expense.repeatWeek"));
    expect(list.textContent).toContain(t("recurring.paused"));
    expect(list.textContent).not.toContain("Apr 1, 2026");
  });
});
