import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "@/lib/i18n/index.svelte";
import { dialogs } from "@/lib/state/dialogs.svelte";
import { openGroup } from "@/lib/state/groups.svelte";
import { navigation } from "@/lib/state/navigation.svelte";
import { session } from "@/lib/state/session.svelte";
import { account, equally, expense, group } from "@/test/fixtures";
import ExpensesTab from "./ExpensesTab.svelte";
import GroupPage from "./GroupPage.svelte";

vi.mock("@/services/api", () => ({ api: { getGroups: vi.fn(async () => []) } }));
vi.mock("@/services/native.svelte", () => ({
  closeTopLayer: () => false,
  isAndroid: false,
  nativeFeatures: { share: false, scan: false, save: false },
  ScanCancelled: class extends Error {},
  scanQrCode: vi.fn(),
}));
vi.mock("svelte-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }));

/** `count` expenses, one a day going back from March 30th. */
const expenses = (count: number) =>
  Array.from({ length: count }, (_, i) =>
    expense({
      id: `e${i}`,
      title: i === 0 ? "Café" : `Expense ${i}`,
      amount_cents: 1000 + i,
      splits: equally("alice", "bob", "carol"),
      created_at: new Date(2026, 2, 30 - i, 12).toISOString(),
    })
  );

const rows = () => screen.queryAllByTestId("expense-item");

beforeEach(() => {
  vi.clearAllMocks();
  window.history.replaceState(null, "");
  session.account = account({ identities: { g1: "alice" } });
  navigation.groupId = "g1";
  navigation.tab = "expenses";
  openGroup.group = group();
  openGroup.balances = [];
  openGroup.settlements = [];
  openGroup.stale = false;
  dialogs.expense = { open: false, editing: null };
});

describe("ExpensesTab", () => {
  it("invites to add the first expense", async () => {
    const { container } = render(ExpensesTab, { group: group() });
    expect(container.textContent).toContain(t("expenses.empty"));
    expect(screen.queryByRole("searchbox")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: t("expenses.addFirst") }));
    expect(dialogs.expense).toEqual({ open: true, editing: null });
  });

  it("lists the latest expenses first, a day at a time, ten to start with", async () => {
    render(ExpensesTab, { group: group({ expenses: expenses(25) }) });
    expect(rows()).toHaveLength(10);
    expect(rows()[0].textContent).toContain("Café");
    expect(screen.getAllByRole("region")).toHaveLength(10);
    await fireEvent.click(
      screen.getByRole("button", { name: new RegExp(t("expenses.loadMore", 10)) })
    );
    expect(rows()).toHaveLength(20);
  });

  it("searches as it is typed, and says when nothing matches", async () => {
    const { container } = render(ExpensesTab, { group: group({ expenses: expenses(25) }) });
    const search = screen.getByRole("searchbox");
    await fireEvent.input(search, { target: { value: "cafe" } });
    expect(rows()).toHaveLength(1);
    expect(container.textContent).toContain(t("expenses.count", 1));

    await fireEvent.input(search, { target: { value: "nothing like it" } });
    expect(rows()).toHaveLength(0);
    expect(container.textContent).toContain(t("expenses.noMatch"));
    await fireEvent.click(screen.getAllByRole("button", { name: t("expenses.clear") })[0]);
    expect(rows()).toHaveLength(10);
  });
});

describe("GroupPage", () => {
  const trip = group({ expenses: expenses(2) });

  it("opens on the expenses, with how many there are on the tab", () => {
    openGroup.group = trip;
    render(GroupPage, { group: trip });
    expect(screen.getByRole("tab", { name: new RegExp(t("tabs.expenses")) }).textContent).toContain(
      "2"
    );
    expect(rows()).toHaveLength(2);
  });

  it("shows one tab at a time", async () => {
    openGroup.group = trip;
    openGroup.balances = [
      {
        participant_id: "alice",
        participant_name: "Alice",
        paid_cents: 2001,
        owed_cents: 667,
        net_cents: 1334,
        removed: false,
      },
    ];
    render(GroupPage, { group: trip });
    await fireEvent.click(screen.getByRole("tab", { name: t("tabs.balances") }));
    expect(navigation.tab).toBe("balances");
    expect(rows()).toHaveLength(0);
    expect(screen.getAllByTestId("balance-card")).toHaveLength(1);

    await fireEvent.click(screen.getByRole("tab", { name: t("tabs.stats") }));
    expect(screen.queryAllByTestId("balance-card")).toHaveLength(0);
    expect(screen.getByTestId("stats-people")).toBeTruthy();
  });

  it("counts who still has to pay on the Settle Up tab", () => {
    openGroup.group = trip;
    openGroup.settlements = [
      { from_id: "bob", from_name: "Bob", to_id: "alice", to_name: "Alice", amount_cents: 667 },
    ];
    render(GroupPage, { group: trip });
    expect(screen.getByRole("tab", { name: new RegExp(t("tabs.settle")) }).textContent).toContain(
      "1"
    );
  });

  it("offers what other devices changed instead of moving the list", async () => {
    openGroup.group = trip;
    const load = vi.spyOn(openGroup, "load").mockResolvedValue();
    const quiet = render(GroupPage, { group: trip });
    expect(screen.queryByRole("button", { name: t("group.refresh") })).toBeNull();
    quiet.unmount();

    openGroup.stale = true;
    render(GroupPage, { group: trip });
    await fireEvent.click(screen.getByRole("button", { name: t("group.refresh") }));
    expect(load).toHaveBeenCalledWith("g1");
  });
});
