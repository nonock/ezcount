import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { joinGroup } from "@/lib/actions/groups";
import { addSelf, chooseIdentity, skipIdentity } from "@/lib/actions/members";
import { t } from "@/lib/i18n/index.svelte";
import { dialogs } from "@/lib/state/dialogs.svelte";
import { openGroup } from "@/lib/state/groups.svelte";
import { session } from "@/lib/state/session.svelte";
import { api } from "@/services/api";
import { account, equally, expense, group, participant } from "@/test/fixtures";
import type { ExpenseHistoryEntry } from "@/types";
import ActivityDialog from "./ActivityDialog.svelte";
import CommentsDialog from "./CommentsDialog.svelte";
import ExpenseHistoryModal from "./ExpenseHistoryModal.svelte";
import FeedbackDialog from "./FeedbackDialog.svelte";
import JoinGroupModal from "./JoinGroupModal.svelte";
import PayDialog from "./PayDialog.svelte";
import WhoAreYouModal from "./WhoAreYouModal.svelte";

// What the device can do, as `native.svelte` reports it.
const device = vi.hoisted(() => ({ features: { share: false, scan: false, save: false } }));

vi.mock("@/services/api", () => ({
  api: { addExpenseComment: vi.fn(), deleteExpenseComment: vi.fn(), sendFeedback: vi.fn() },
}));
vi.mock("@/services/native.svelte", () => ({
  nativeFeatures: device.features,
  closeTopLayer: () => false,
}));
vi.mock("@/lib/actions/groups", () => ({ joinGroup: vi.fn() }));
vi.mock("@/lib/actions/scan", () => ({ scanInvite: vi.fn() }));
vi.mock("@/lib/actions/members", () => ({
  addSelf: vi.fn(),
  chooseIdentity: vi.fn(),
  skipIdentity: vi.fn(),
}));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "1.2.3") }));
vi.mock("svelte-sonner", () => ({ toast: { success: vi.fn(), info: vi.fn() } }));

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
  device.features.scan = false;
  session.account = account({ identities: { g1: "bob" } });
  openGroup.group = group();
  dialogs.join = { open: false, code: "", error: null };
  dialogs.comments = null;
  dialogs.history = null;
  dialogs.activity = false;
  dialogs.feedback = false;
  dialogs.who = false;
  dialogs.pay = { open: false, fromId: "", toId: "", amountCents: 0 };
  dialogs.reimburse = { open: false, fromId: "", toId: "", amount: "" };
  vi.spyOn(openGroup, "change").mockImplementation(async (apply) => {
    await apply("g1");
  });
});

describe("JoinGroupModal", () => {
  it("opens with the invite it was given, and why joining with it failed", async () => {
    dialogs.openJoin("https://relay.example.com/join#g=g2", "Group not found");
    render(JoinGroupModal);
    const dialog = await screen.findByRole("dialog");
    const link = screen.getByLabelText(t("join.link")) as HTMLTextAreaElement;
    expect(link.value).toBe("https://relay.example.com/join#g=g2");
    expect(dialog.textContent).toContain("Group not found");
  });

  it("joins with the link typed and closes", async () => {
    vi.mocked(joinGroup).mockResolvedValue(undefined);
    dialogs.openJoin();
    render(JoinGroupModal);
    await type(await screen.findByLabelText(t("join.link")), "  invite  ");
    await submit();
    expect(joinGroup).toHaveBeenCalledWith("invite");
    await vi.waitFor(() => expect(dialogs.join.open).toBe(false));
  });

  it("stays open and says why joining failed, and does nothing without a link", async () => {
    vi.mocked(joinGroup).mockRejectedValue(new Error("This is not a valid ezcount invite"));
    dialogs.openJoin();
    render(JoinGroupModal);
    await submit();
    expect(joinGroup).not.toHaveBeenCalled();
    await type(await screen.findByLabelText(t("join.link")), "nope");
    await submit();
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() =>
      expect(dialog.textContent).toContain("This is not a valid ezcount invite")
    );
    expect(dialogs.join.open).toBe(true);
  });

  it("offers to scan only where the device can", async () => {
    dialogs.openJoin();
    const without = render(JoinGroupModal);
    await screen.findByRole("dialog");
    expect(screen.queryByRole("button", { name: t("join.scan") })).toBeNull();
    without.unmount();

    device.features.scan = true;
    render(JoinGroupModal);
    expect(await screen.findByRole("button", { name: t("join.scan") })).toBeTruthy();
  });
});

describe("CommentsDialog", () => {
  const commented = group({
    expenses: [
      expense({
        comments: [
          { id: "c1", text: "Receipt?", created_at: "2026-03-05T10:00:00Z", by: "alice" },
          { id: "c2", text: "Here it is", created_at: "2026-03-05T11:00:00Z", by: "bob" },
          { id: "c3", text: "From before names", created_at: "2026-03-05T12:00:00Z" },
        ],
      }),
    ],
  });

  it("says there is none yet", async () => {
    dialogs.comments = "e1";
    render(CommentsDialog, { group: group({ expenses: [expense()] }) });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("Taxi");
    expect(dialog.textContent).toContain(t("comments.none"));
  });

  it("lists what was written and by whom", async () => {
    dialogs.comments = "e1";
    render(CommentsDialog, { group: commented });
    const list = await screen.findByRole("list", { name: t("comments.title") });
    const texts = within(list)
      .getAllByRole("listitem")
      .map((li) => li.textContent ?? "");
    expect(texts[0]).toContain("Alice");
    expect(texts[0]).toContain("Receipt?");
    expect(texts[2]).toContain(t("comments.someone"));
  });

  it("lets the user delete their own comments, and those of no one", async () => {
    dialogs.comments = "e1";
    render(CommentsDialog, { group: commented });
    const list = await screen.findByRole("list", { name: t("comments.title") });
    const [alices, bobs, unknown] = within(list).getAllByRole("listitem");
    const remove = { name: t("comments.delete") };
    expect(within(alices).queryByRole("button", remove)).toBeNull();
    expect(within(unknown).getByRole("button", remove)).toBeTruthy();
    await fireEvent.click(within(bobs).getByRole("button", remove));
    expect(api.deleteExpenseComment).toHaveBeenCalledWith("g1", "c2");
  });

  it("adds what is typed and empties the field", async () => {
    dialogs.comments = "e1";
    render(CommentsDialog, { group: commented });
    const field = (await screen.findByLabelText(t("comments.write"))) as HTMLInputElement;
    expect(screen.getByRole("button", { name: t("comments.send") })).toHaveProperty(
      "disabled",
      true
    );
    await type(field, "  Thanks  ");
    await submit();
    expect(api.addExpenseComment).toHaveBeenCalledWith("g1", "e1", "Thanks");
    await vi.waitFor(() => expect(field.value).toBe(""));
  });

  it("says why a comment wasn't added", async () => {
    vi.mocked(api.addExpenseComment).mockRejectedValue(new Error("A comment can't be empty"));
    dialogs.comments = "e1";
    render(CommentsDialog, { group: commented });
    await type(await screen.findByLabelText(t("comments.write")), "x");
    await submit();
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() => expect(dialog.textContent).toContain("A comment can't be empty"));
  });
});

describe("ActivityDialog", () => {
  const busy = group({
    participants: [
      participant("alice"),
      participant("bob", { added_at: "2026-02-01T00:00:00Z", added_by: "alice" }),
    ],
    expenses: [expense({ added_at: "2026-03-01T10:00:00Z", added_by: "bob" })],
  });

  it("lists what happened, the latest first", async () => {
    dialogs.activity = true;
    render(ActivityDialog, { group: busy });
    const list = await screen.findByRole("list", { name: t("activity.title") });
    const texts = within(list)
      .getAllByRole("listitem")
      .map((li) => li.textContent ?? "");
    expect(texts).toHaveLength(3);
    expect(texts[0]).toContain(t("activity.addedBy", "Bob", "Taxi"));
    expect(texts[0]).toContain("€30");
    expect(texts[1]).toContain(t("members.addedBy", "Bob", "Alice"));
  });

  it("shows only the expenses, or only the members", async () => {
    dialogs.activity = true;
    render(ActivityDialog, { group: busy });
    const list = await screen.findByRole("list", { name: t("activity.title") });
    await fireEvent.click(screen.getByRole("tab", { name: t("activity.members") }));
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    await fireEvent.click(screen.getByRole("tab", { name: t("activity.expenses") }));
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);
  });

  it("shows the older events when asked", async () => {
    const many = group({
      expenses: Array.from({ length: 40 }, (_, i) =>
        expense({ id: `e${i}`, added_at: `2026-03-01T10:${String(i).padStart(2, "0")}:00Z` })
      ),
    });
    dialogs.activity = true;
    render(ActivityDialog, { group: many });
    const list = await screen.findByRole("list", { name: t("activity.title") });
    expect(within(list).getAllByRole("listitem")).toHaveLength(30);
    await fireEvent.click(screen.getByRole("button", { name: t("activity.more") }));
    expect(within(list).getAllByRole("listitem")).toHaveLength(41);
    expect(screen.queryByRole("button", { name: t("activity.more") })).toBeNull();
  });
});

describe("PayDialog", () => {
  const iban = "FR7630006000011234567890189";
  const withIban = (currency = "EUR") =>
    group({ currency, participants: [participant("alice", { iban }), participant("bob")] });

  it("shows the transfer to make, with a QR code banking apps scan", async () => {
    dialogs.openPay("bob", "alice", 1500);
    render(PayDialog, { group: withIban() });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("pay.title", "Alice"));
    expect(screen.getByTestId("transfer-iban").textContent).toBe(
      "FR76 3000 6000 0112 3456 7890 189"
    );
    expect(dialog.textContent).toContain("€15");
    const qr = screen.getByTestId("transfer-qr");
    expect(qr.getAttribute("alt")).toBe(t("pay.qrAlt", "Alice"));
    expect(qr.getAttribute("src")).toMatch(/^data:image\/svg\+xml/);
  });

  it("has no QR code outside euros: the format is for SEPA transfers", async () => {
    dialogs.openPay("bob", "alice", 1500);
    render(PayDialog, { group: withIban("USD") });
    await screen.findByRole("dialog");
    expect(screen.queryByTestId("transfer-qr")).toBeNull();
    expect(screen.getByTestId("transfer-iban")).toBeTruthy();
  });

  it("copies the IBAN", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    dialogs.openPay("bob", "alice", 1500);
    render(PayDialog, { group: withIban() });
    await fireEvent.click(await screen.findByRole("button", { name: t("pay.copy") }));
    expect(writeText).toHaveBeenCalledWith(iban);
    await vi.waitFor(() => expect(toast.success).toHaveBeenCalledWith(t("pay.copied")));
  });

  it("goes on to record the payment once made", async () => {
    dialogs.openPay("bob", "alice", 1500);
    render(PayDialog, { group: withIban() });
    await fireEvent.click(await screen.findByRole("button", { name: t("settle.markPaid") }));
    expect(dialogs.pay.open).toBe(false);
    expect(dialogs.reimburse).toEqual({ open: true, fromId: "bob", toId: "alice", amount: "15" });
  });
});

describe("ExpenseHistoryModal", () => {
  const edit = (more: Partial<ExpenseHistoryEntry>): ExpenseHistoryEntry => ({
    edited_at: "2026-03-02T10:00:00Z",
    previous_title: "Cab",
    previous_amount_cents: 2000,
    previous_paid_by: "bob",
    previous_splits: equally("bob"),
    summary: "Title changed from 'Cab' to 'Taxi'",
    ...more,
  });

  it("says an expense was never changed", async () => {
    dialogs.history = expense();
    render(ExpenseHistoryModal, { group: group() });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("history.none"));
    const current = screen.getByRole("region", { name: t("history.current") });
    expect(current.textContent).toContain("Taxi");
    expect(current.textContent).toContain("€30");
  });

  it("lists the edits, the latest first, with what the expense was before", async () => {
    dialogs.history = expense({
      history: [
        edit({}),
        edit({
          edited_at: "2026-03-03T10:00:00Z",
          previous_title: "Taxi",
          previous_amount_cents: 2500,
          summary: "Amount changed from 25.00 to 30.00",
        }),
      ],
    });
    render(ExpenseHistoryModal, { group: group() });
    const dialog = await screen.findByRole("dialog");
    const revisions = [...dialog.querySelectorAll("ol > li")].map((li) => li.textContent ?? "");
    expect(revisions).toHaveLength(2);
    expect(revisions[0]).toContain(t("history.revision", 2));
    expect(revisions[0]).toContain("Amount changed from 25.00 to 30.00");
    expect(revisions[1]).toContain(t("history.revision", 1));
    expect(revisions[1]).toContain("Cab");
    expect(revisions[1]).toContain("Bob");
  });

  it("names several payers with what each paid, in the currency paid", async () => {
    dialogs.history = expense({
      amount_cents: 9000,
      original: { currency: "USD", amount_cents: 10000, rate: "0.9" },
      payers: [
        { participant_id: "alice", amount_cents: 7500 },
        { participant_id: "bob", amount_cents: 2500 },
      ],
      splits: [
        { participant_id: "alice", shares: 2 },
        { participant_id: "bob", shares: 0, fixed_cents: 1000 },
      ],
    });
    render(ExpenseHistoryModal, { group: group() });
    await screen.findByRole("dialog");
    const current = screen.getByRole("region", { name: t("history.current") });
    expect(current.textContent).toContain("Alice ($75) and Bob ($25)");
    expect(current.textContent).toContain(t("common.parts", 2));
    expect(current.textContent).toContain("$10");
  });
});

describe("FeedbackDialog", () => {
  it("sends the message with the contact and which app it comes from", async () => {
    vi.mocked(api.sendFeedback).mockResolvedValue(undefined);
    dialogs.feedback = true;
    render(FeedbackDialog);
    const send = await screen.findByRole("button", { name: t("feedback.send") });
    expect(send).toHaveProperty("disabled", true);
    await type(screen.getByLabelText(t("feedback.message")), " Dark mode please ");
    await type(screen.getByLabelText(t("feedback.contact")), "alice@example.com");
    await submit();
    await vi.waitFor(() => expect(api.sendFeedback).toHaveBeenCalled());
    const [message, contact, app] = vi.mocked(api.sendFeedback).mock.calls[0];
    expect(message).toBe("Dark mode please");
    expect(contact).toBe("alice@example.com");
    expect(app).toContain("ezcount 1.2.3");
    await vi.waitFor(() => expect(dialogs.feedback).toBe(false));
    expect(toast.success).toHaveBeenCalledWith(t("feedback.sent"));
  });

  it("sends no contact when none was given, and says why sending failed", async () => {
    vi.mocked(api.sendFeedback).mockRejectedValue(
      new Error("This sync server doesn't take messages yet")
    );
    dialogs.feedback = true;
    render(FeedbackDialog);
    await type(await screen.findByLabelText(t("feedback.message")), "Hello");
    await submit();
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() =>
      expect(dialog.textContent).toContain("This sync server doesn't take messages yet")
    );
    expect(vi.mocked(api.sendFeedback).mock.calls[0][1]).toBeNull();
    expect(dialogs.feedback).toBe(true);
  });
});

describe("WhoAreYouModal", () => {
  const trip = group({
    participants: [
      participant("alice"),
      participant("bob"),
      participant("carol", { removed: true }),
    ],
  });

  it("offers the members still in the group, marking who the user already is", async () => {
    dialogs.who = true;
    render(WhoAreYouModal, { group: trip });
    const list = await screen.findByRole("list", { name: t("common.members") });
    expect(within(list).getAllByRole("button")).toHaveLength(2);
    const alice = within(list).getByRole("button", { name: "Alice" });
    const bob = within(list).getByRole("button", { name: /Bob/ });
    expect(within(alice).queryByLabelText(t("common.you"))).toBeNull();
    expect(within(bob).getByLabelText(t("common.you"))).toBeTruthy();
  });

  it("records who the user says they are and closes", async () => {
    vi.mocked(chooseIdentity).mockResolvedValue(undefined);
    dialogs.who = true;
    render(WhoAreYouModal, { group: trip });
    await fireEvent.click(await screen.findByRole("button", { name: "Alice" }));
    expect(chooseIdentity).toHaveBeenCalledWith("alice");
    await vi.waitFor(() => expect(dialogs.who).toBe(false));
  });

  it("adds the user under a name when they aren't in the list", async () => {
    vi.mocked(addSelf).mockResolvedValue(undefined);
    session.account = account({ display_name: "Dan" });
    dialogs.who = true;
    render(WhoAreYouModal, { group: trip });
    await fireEvent.click(await screen.findByRole("button", { name: t("who.notListed") }));
    const name = screen.getByLabelText(t("who.yourName")) as HTMLInputElement;
    expect(name.value).toBe("Dan");
    await type(name, " Daniel ");
    await submit();
    expect(addSelf).toHaveBeenCalledWith("Daniel");
  });

  it("stays open and says why the answer wasn't kept", async () => {
    vi.mocked(chooseIdentity).mockRejectedValue(
      new Error("This person is not a member of the group")
    );
    dialogs.who = true;
    render(WhoAreYouModal, { group: trip });
    await fireEvent.click(await screen.findByRole("button", { name: "Alice" }));
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() =>
      expect(dialog.textContent).toContain("This person is not a member of the group")
    );
    expect(dialogs.who).toBe(true);
    expect(skipIdentity).not.toHaveBeenCalled();
  });
});
