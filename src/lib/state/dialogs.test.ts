import { describe, expect, it } from "vitest";
import { expense } from "@/test/fixtures";
import { dialogs } from "./dialogs.svelte";

describe("dialogs", () => {
  it("opens the join dialog empty, or with an invite and why it failed", () => {
    dialogs.openJoin();
    expect(dialogs.join).toEqual({ open: true, code: "", error: null });
    dialogs.openJoin("invite", "Group not found");
    expect(dialogs.join).toEqual({ open: true, code: "invite", error: "Group not found" });
  });

  it("opens the expense form to add, or to edit an expense", () => {
    dialogs.openExpense();
    expect(dialogs.expense).toEqual({ open: true, editing: null });
    const taxi = expense();
    dialogs.openExpense(taxi);
    expect(dialogs.expense.editing).toEqual(taxi);
  });

  it("opens the bank transfer of what one member owes another", () => {
    dialogs.openPay("bob", "alice", 1500);
    expect(dialogs.pay).toEqual({ open: true, fromId: "bob", toId: "alice", amountCents: 1500 });
  });

  it("opens a payment with what is known of it", () => {
    dialogs.openReimburse();
    expect(dialogs.reimburse).toEqual({ open: true, fromId: "", toId: "", amount: "" });
    dialogs.openReimburse({ fromId: "bob", amount: "15" });
    expect(dialogs.reimburse).toEqual({ open: true, fromId: "bob", toId: "", amount: "15" });
  });
});
