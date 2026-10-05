import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createGroup } from "@/lib/actions/groups";
import { t } from "@/lib/i18n/index.svelte";
import { dialogs } from "@/lib/state/dialogs.svelte";
import { openGroup } from "@/lib/state/groups.svelte";
import { session } from "@/lib/state/session.svelte";
import { api } from "@/services/api";
import { account, group, participant } from "@/test/fixtures";
import CreateGroupModal from "./CreateGroupModal.svelte";
import MemberModal from "./MemberModal.svelte";
import RecordReimbursementModal from "./RecordReimbursementModal.svelte";

vi.mock("@/lib/actions/groups", () => ({ createGroup: vi.fn() }));
vi.mock("@/services/api", () => ({ api: { recordReimbursement: vi.fn() } }));
vi.mock("@/services/native.svelte", () => ({ closeTopLayer: () => false }));

/** Types in a field, as Svelte hears it. */
async function type(field: HTMLElement, value: string) {
  await fireEvent.input(field, { target: { value } });
}

/** Submits the form of the dialog on screen. */
async function submit() {
  const form = (await screen.findByRole("dialog")).querySelector("form");
  if (!form) throw new Error("The dialog has no form");
  await fireEvent.submit(form);
}

beforeEach(() => {
  vi.clearAllMocks();
  session.account = account();
  openGroup.group = group();
  dialogs.createGroup = false;
  dialogs.reimburse = { open: false, fromId: "", toId: "", amount: "" };
  vi.spyOn(openGroup, "change").mockImplementation(async (apply) => {
    await apply("g1");
  });
});

describe("MemberModal", () => {
  it("adds a member under the name typed", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(MemberModal, { open: true, onSubmit });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("member.addTitle"));
    await type(screen.getByLabelText(t("common.name")), "  Dan ");
    await submit();
    expect(onSubmit).toHaveBeenCalledWith("Dan");
    await vi.waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("doesn't add a member without a name", async () => {
    const onSubmit = vi.fn();
    render(MemberModal, { open: true, onSubmit });
    await type(await screen.findByLabelText(t("common.name")), "   ");
    await submit();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("starts from a member's name to rename them, and offers to remove them", async () => {
    const onRemove = vi.fn();
    render(MemberModal, {
      open: true,
      member: participant("bob"),
      onSubmit: vi.fn(),
      onRemove,
    });
    const name = (await screen.findByLabelText(t("common.name"))) as HTMLInputElement;
    expect(name.value).toBe("Bob");
    expect(screen.getByRole("button", { name: t("common.rename") })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: t("member.remove") }));
    expect(onRemove).toHaveBeenCalled();
  });

  it("stays open and says why when the name is refused", async () => {
    const onSubmit = vi.fn().mockRejectedValue(new Error("Participant name cannot be empty"));
    render(MemberModal, { open: true, onSubmit });
    await type(await screen.findByLabelText(t("common.name")), "Dan");
    await submit();
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() =>
      expect(dialog.textContent).toContain("Participant name cannot be empty")
    );
  });
});

describe("CreateGroupModal", () => {
  it("suggests the user's own name as the first member", async () => {
    session.account = account({ display_name: "Alice M." });
    dialogs.createGroup = true;
    render(CreateGroupModal);
    const own = (await screen.findByLabelText(t("create.yourName"))) as HTMLInputElement;
    expect(own.value).toBe("Alice M.");
  });

  it("needs a name for the group and for the user", async () => {
    dialogs.createGroup = true;
    render(CreateGroupModal);
    await submit();
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("create.needName"));
    await type(screen.getByLabelText(t("create.name")), "Trip");
    await type(screen.getByLabelText(t("create.yourName")), " ");
    await submit();
    expect(dialog.textContent).toContain(t("create.needYourName"));
    expect(createGroup).not.toHaveBeenCalled();
  });

  it("makes the group with the people named, the user first", async () => {
    vi.mocked(createGroup).mockResolvedValue(undefined);
    dialogs.createGroup = true;
    render(CreateGroupModal);
    await type(await screen.findByLabelText(t("create.name")), " Trip ");
    await type(screen.getByLabelText(t("create.participant", 2)), "Bob");
    await submit();
    expect(createGroup).toHaveBeenCalledWith("Trip", "EUR", ["alice", "Bob"]);
    await vi.waitFor(() => expect(dialogs.createGroup).toBe(false));
  });

  it("adds and removes people, but never the user", async () => {
    dialogs.createGroup = true;
    render(CreateGroupModal);
    await screen.findByRole("dialog");
    await fireEvent.click(screen.getByRole("button", { name: t("create.addPerson") }));
    expect(screen.getByLabelText(t("create.participant", 4))).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: t("create.removeParticipant", 2) }));
    expect(screen.queryByLabelText(t("create.participant", 4))).toBeNull();
    expect(screen.queryByRole("button", { name: t("create.removeParticipant", 1) })).toBeNull();
  });

  it("says why the group couldn't be made", async () => {
    vi.mocked(createGroup).mockRejectedValue(new Error("Group name cannot be empty"));
    dialogs.createGroup = true;
    render(CreateGroupModal);
    await type(await screen.findByLabelText(t("create.name")), "Trip");
    await submit();
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() => expect(dialog.textContent).toContain("Group name cannot be empty"));
    expect(dialogs.createGroup).toBe(true);
  });
});

describe("RecordReimbursementModal", () => {
  it("records the payment it was opened with", async () => {
    dialogs.openReimburse({ fromId: "bob", toId: "alice", amount: "15" });
    render(RecordReimbursementModal, { group: group() });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("Bob");
    await type(screen.getByLabelText(t("reimburse.note")), " cash ");
    await submit();
    expect(api.recordReimbursement).toHaveBeenCalledWith("g1", "bob", "alice", 1500, "cash");
    await vi.waitFor(() => expect(dialogs.reimburse.open).toBe(false));
  });

  it("starts from the first two members, and needs an amount", async () => {
    dialogs.openReimburse();
    render(RecordReimbursementModal, { group: group() });
    await submit();
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("expense.needAmount"));
    await type(screen.getByLabelText(t("common.amount")), "7.5");
    await submit();
    expect(api.recordReimbursement).toHaveBeenCalledWith("g1", "alice", "bob", 750, undefined);
  });

  it("refuses a payment from someone to themselves", async () => {
    dialogs.openReimburse({ fromId: "bob", toId: "bob", amount: "15" });
    render(RecordReimbursementModal, { group: group() });
    await submit();
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain(t("reimburse.samePerson"));
    expect(api.recordReimbursement).not.toHaveBeenCalled();
  });

  it("stays open and says why the payment was refused", async () => {
    vi.mocked(api.recordReimbursement).mockRejectedValue(new Error("Amount is too large"));
    dialogs.openReimburse({ fromId: "bob", toId: "alice", amount: "15" });
    render(RecordReimbursementModal, { group: group() });
    await submit();
    const dialog = await screen.findByRole("dialog");
    await vi.waitFor(() => expect(dialog.textContent).toContain("Amount is too large"));
    expect(dialogs.reimburse.open).toBe(true);
  });
});
