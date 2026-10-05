import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { answerConfirm } from "@/test/confirm";
import { expense, group } from "@/test/fixtures";
import { pressAction } from "@/test/toast";
import { confirmation } from "../state/confirm.svelte";
import { openGroup } from "../state/groups.svelte";
import { deleteExpense, purgeExpense, restoreExpense, stopRecurring } from "./expenses";

vi.mock("@/services/api", () => ({
  api: {
    deleteExpense: vi.fn(),
    restoreExpense: vi.fn(),
    purgeExpense: vi.fn(),
    stopRecurringExpense: vi.fn(),
  },
}));
vi.mock("@/services/native.svelte", () => ({ closeTopLayer: () => false }));
vi.mock("svelte-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  confirmation.settle(false);
  openGroup.group = group({
    expenses: [expense()],
    trash: [{ expense: expense({ id: "gone", title: "Hotel" }), deleted_at: "2026-03-06" }],
    recurring: [
      {
        id: "rent",
        title: "Rent",
        amount_cents: 90000,
        paid_by: "alice",
        splits: [],
        every: "month",
        next: "2026-04-01",
      },
    ],
  });
  // The change itself is the state's own, tested with it.
  vi.spyOn(openGroup, "change").mockImplementation(async (apply) => {
    await apply("g1");
  });
});

describe("deleteExpense", () => {
  it("moves the expense to the trash without asking, and offers to undo", async () => {
    await deleteExpense("e1");
    expect(api.deleteExpense).toHaveBeenCalledWith("g1", "e1");
    const [message, options] = vi.mocked(toast.success).mock.calls[0];
    expect(message).toContain("Taxi");
    pressAction(options);
    expect(api.restoreExpense).toHaveBeenCalledWith("g1", "e1");
  });

  it("does nothing for an expense the group no longer has", async () => {
    await deleteExpense("nope");
    expect(api.deleteExpense).not.toHaveBeenCalled();
  });

  it("says why it failed", async () => {
    vi.mocked(api.deleteExpense).mockRejectedValue(new Error("Expense not found"));
    await deleteExpense("e1");
    expect(toast.error).toHaveBeenCalledWith(expect.any(String), {
      description: "Expense not found",
    });
    expect(toast.success).not.toHaveBeenCalled();
  });
});

describe("restoreExpense", () => {
  it("puts the expense back, or says why it couldn't", async () => {
    await restoreExpense("gone");
    expect(api.restoreExpense).toHaveBeenCalledWith("g1", "gone");
    vi.mocked(api.restoreExpense).mockRejectedValue(
      new Error("This expense is no longer in the trash")
    );
    await restoreExpense("gone");
    expect(toast.error).toHaveBeenCalledTimes(1);
  });
});

describe("purgeExpense", () => {
  it("asks before removing a deleted expense for good", async () => {
    const done = purgeExpense("gone");
    const asked = await answerConfirm(true);
    await done;
    expect(asked.title).toContain("Hotel");
    expect(asked.destructive).toBe(true);
    expect(api.purgeExpense).toHaveBeenCalledWith("g1", "gone");
  });

  it("keeps it when the user says no", async () => {
    const done = purgeExpense("gone");
    await answerConfirm(false);
    await done;
    expect(api.purgeExpense).not.toHaveBeenCalled();
  });

  it("only knows what is in the trash", async () => {
    await purgeExpense("e1");
    expect(confirmation.open).toBe(false);
  });
});

describe("stopRecurring", () => {
  it("asks before stopping a repeated expense", async () => {
    const done = stopRecurring("rent");
    const asked = await answerConfirm(true);
    await done;
    expect(asked.title).toContain("Rent");
    expect(api.stopRecurringExpense).toHaveBeenCalledWith("g1", "rent");
  });

  it("leaves it when the user says no, or when it no longer exists", async () => {
    const done = stopRecurring("rent");
    await answerConfirm(false);
    await done;
    await stopRecurring("nope");
    expect(api.stopRecurringExpense).not.toHaveBeenCalled();
  });
});
