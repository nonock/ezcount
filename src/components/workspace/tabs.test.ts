import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { deleteGroup, refuseDeletion } from "@/lib/actions/groups";
import { t } from "@/lib/i18n/index.svelte";
import { dialogs } from "@/lib/state/dialogs.svelte";
import { openGroup } from "@/lib/state/groups.svelte";
import { session } from "@/lib/state/session.svelte";
import { account, expense, group, participant } from "@/test/fixtures";
import type { ParticipantBalance, SettlementTransfer } from "@/types";
import BalancesTab from "./BalancesTab.svelte";
import DeletionRequest from "./DeletionRequest.svelte";
import SettleUpTab from "./SettleUpTab.svelte";

vi.mock("@/lib/actions/groups", () => ({ deleteGroup: vi.fn(), refuseDeletion: vi.fn() }));
vi.mock("@/services/api", () => ({ api: {} }));
vi.mock("@/services/native.svelte", () => ({ closeTopLayer: () => false }));

const balance = (id: string, net: number, more: Partial<ParticipantBalance> = {}) => ({
  participant_id: id,
  participant_name: id.charAt(0).toUpperCase() + id.slice(1),
  paid_cents: Math.max(net, 0),
  owed_cents: Math.max(-net, 0),
  net_cents: net,
  removed: false,
  ...more,
});

const bobPaysAlice: SettlementTransfer = {
  from_id: "bob",
  from_name: "Bob",
  to_id: "alice",
  to_name: "Alice",
  amount_cents: 1500,
};

beforeEach(() => {
  vi.clearAllMocks();
  openGroup.group = group();
  openGroup.balances = [];
  openGroup.settlements = [];
  session.account = account();
  dialogs.reimburse = { open: false, fromId: "", toId: "", amount: "" };
  dialogs.pay.open = false;
});

describe("BalancesTab", () => {
  it("says who gets money back, who owes and who is settled", () => {
    openGroup.balances = [balance("alice", 1500), balance("bob", -1500), balance("carol", 0)];
    render(BalancesTab, { group: group() });
    const cards = screen.getAllByTestId("balance-card").map((card) => card.textContent ?? "");
    expect(cards[0]).toContain(t("balances.getsBack"));
    expect(cards[0]).toContain("+€15");
    expect(cards[1]).toContain(t("balances.owes"));
    expect(cards[1]).toContain("-€15");
    expect(cards[2]).toContain(t("balances.settled"));
  });

  it("offers who owes to pay it back, with the amount filled in", async () => {
    openGroup.balances = [balance("alice", 1500), balance("bob", -1500)];
    render(BalancesTab, { group: group() });
    const buttons = screen.getAllByRole("button", { name: new RegExp(t("balances.reimburse")) });
    expect(buttons).toHaveLength(1);
    await fireEvent.click(buttons[0]);
    expect(dialogs.reimburse).toEqual({ open: true, fromId: "bob", toId: "", amount: "15" });
  });

  it("marks someone who left and still has something to settle", () => {
    openGroup.balances = [balance("dan", -500, { removed: true })];
    const { container } = render(BalancesTab, { group: group() });
    expect(container.textContent).toContain(t("member.removed"));
  });

  it("fills each bar in proportion to the largest balance", () => {
    openGroup.balances = [balance("alice", 2000), balance("bob", -500)];
    render(BalancesTab, { group: group() });
    const bars = screen.getAllByRole("progressbar");
    expect(bars.map((bar) => bar.getAttribute("aria-valuenow"))).toEqual(["100", "25"]);
  });
});

describe("SettleUpTab", () => {
  const spent = group({ expenses: [expense()] });

  it("says so when nobody owes anything", () => {
    const { container } = render(SettleUpTab, { group: spent });
    expect(container.textContent).toContain(t("settle.allSettled"));
    expect(screen.queryByRole("button", { name: t("settle.markPaid") })).toBeNull();
  });

  it("lists who pays whom, and records the payment as shown", async () => {
    openGroup.settlements = [bobPaysAlice];
    const { container } = render(SettleUpTab, { group: spent });
    expect(container.textContent).toContain("Bob");
    expect(container.textContent).toContain("€15");
    await fireEvent.click(screen.getByRole("button", { name: t("settle.markPaid") }));
    expect(dialogs.reimburse).toEqual({ open: true, fromId: "bob", toId: "alice", amount: "15" });
  });

  it("offers a bank transfer only to someone who gave their IBAN", async () => {
    openGroup.settlements = [bobPaysAlice];
    const plain = render(SettleUpTab, { group: spent });
    expect(screen.queryByRole("button", { name: t("pay.label", "Alice") })).toBeNull();
    plain.unmount();

    const withIban = group({
      expenses: [expense()],
      participants: [
        participant("alice", { iban: "FR7630006000011234567890189" }),
        participant("bob"),
      ],
    });
    render(SettleUpTab, { group: withIban });
    await fireEvent.click(screen.getByRole("button", { name: t("pay.label", "Alice") }));
    expect(dialogs.pay).toEqual({ open: true, fromId: "bob", toId: "alice", amountCents: 1500 });
  });
});

describe("DeletionRequest", () => {
  it("shows nothing while nobody asked to delete the group", () => {
    const { container } = render(DeletionRequest, { group: group() });
    expect(container.textContent?.trim()).toBe("");
  });

  it("says who agreed and who hasn't, and takes the user's answer", async () => {
    session.account = account({ identities: { g1: "alice" } });
    const asked = group({ deletion_votes: ["bob"] });
    const { container } = render(DeletionRequest, { group: asked });
    expect(container.textContent).toContain(t("deletion.asked", "Bob", 1));
    expect(container.textContent).toContain(t("deletion.waiting", "Alice and Carol"));
    await fireEvent.click(screen.getByRole("button", { name: t("deletion.agree") }));
    expect(deleteGroup).toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: t("deletion.refuse") }));
    expect(refuseDeletion).toHaveBeenCalled();
  });

  it("lets who agreed take it back", async () => {
    session.account = account({ identities: { g1: "bob" } });
    render(DeletionRequest, { group: group({ deletion_votes: ["bob"] }) });
    expect(screen.queryByRole("button", { name: t("deletion.agree") })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: t("deletion.takeBack") }));
    expect(refuseDeletion).toHaveBeenCalled();
  });
});
