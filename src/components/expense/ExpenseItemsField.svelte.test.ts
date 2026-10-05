import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import { type ItemState, newItem } from "@/lib/expenseForm.svelte";
import { t } from "@/lib/i18n/index.svelte";
import { participant } from "@/test/fixtures";
import ExpenseItemsField from "./ExpenseItemsField.svelte";

const people = [participant("alice"), participant("bob"), participant("carol", { removed: true })];

/** The field over these lines, held as the form holds them: in reactive state. */
function show(lines: ItemState[]) {
  const items = $state(lines);
  const onChange = vi.fn();
  const view = render(ExpenseItemsField, { items, participants: people, onChange });
  return { onChange, ...view };
}

const line = (n: number) => screen.getByRole("listitem", { name: t("expense.itemLabel", n) });

describe("ExpenseItemsField", () => {
  it("shows each line with its name, its amount and who it is for", () => {
    show([newItem(["alice"], "Wine", "12")]);
    const name = screen.getByLabelText(t("expense.itemNameLabel", 1)) as HTMLInputElement;
    expect(name.value).toBe("Wine");
    const pressed = within(line(1))
      .getAllByRole("button", { pressed: true })
      .map((button) => button.textContent?.trim());
    expect(pressed).toEqual(["Alice"]);
  });

  it("adds a line for everyone still in the group", async () => {
    show([]);
    await fireEvent.click(screen.getByRole("button", { name: t("expense.addItem") }));
    const pressed = within(line(1))
      .getAllByRole("button", { pressed: true })
      .map((button) => button.textContent?.trim());
    expect(pressed).toEqual(["Alice", "Bob"]);
  });

  it("takes someone off a line and puts them back, in the group's order", async () => {
    show([newItem(["alice", "bob"], "Wine", "12")]);
    const who = () =>
      within(line(1))
        .getAllByRole("button", { pressed: true })
        .map((button) => button.textContent?.trim());
    await fireEvent.click(within(line(1)).getByRole("button", { name: "Alice" }));
    expect(who()).toEqual(["Bob"]);
    await fireEvent.click(within(line(1)).getByRole("button", { name: "Alice" }));
    expect(who()).toEqual(["Alice", "Bob"]);
  });

  it("tells the form when an amount changes or a line goes", async () => {
    const { onChange } = show([newItem(["alice"], "Wine", "12"), newItem(["bob"], "Bread", "3")]);
    await fireEvent.input(screen.getByLabelText(t("expense.itemAmountLabel", 1)), {
      target: { value: "15" },
    });
    expect(onChange).toHaveBeenCalledTimes(1);
    await fireEvent.click(screen.getByRole("button", { name: t("expense.removeItem", 1) }));
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    expect((screen.getByLabelText(t("expense.itemNameLabel", 1)) as HTMLInputElement).value).toBe(
      "Bread"
    );
  });
});
