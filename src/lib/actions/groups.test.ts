import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { answerConfirm } from "@/test/confirm";
import { account, expense, group } from "@/test/fixtures";
import type { ParticipantBalance } from "@/types";
import { t } from "../i18n/index.svelte";
import { confirmation } from "../state/confirm.svelte";
import { dialogs } from "../state/dialogs.svelte";
import { groupList, openGroup } from "../state/groups.svelte";
import { navigation } from "../state/navigation.svelte";
import { session } from "../state/session.svelte";
import {
  createGroup,
  deleteGroup,
  goHome,
  importGroup,
  joinGroup,
  leaveGroup,
  refuseDeletion,
  setArchived,
} from "./groups";

vi.mock("@/services/api", () => ({
  api: {
    getGroups: vi.fn(),
    getAccount: vi.fn(),
    createGroup: vi.fn(),
    joinGroup: vi.fn(),
    importGroupCsv: vi.fn(),
    leaveGroup: vi.fn(),
    deleteGroup: vi.fn(),
    refuseGroupDeletion: vi.fn(),
    setGroupArchived: vi.fn(),
  },
}));
vi.mock("@/services/native.svelte", () => ({ closeTopLayer: () => false }));
vi.mock("svelte-sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

const balance = (id: string, net: number): ParticipantBalance => ({
  participant_id: id,
  participant_name: id,
  paid_cents: 0,
  owed_cents: 0,
  net_cents: net,
  removed: false,
});
const owing = [balance("alice", 1500), balance("bob", -1500), balance("carol", 0)];

beforeEach(() => {
  vi.clearAllMocks();
  confirmation.settle(false);
  window.history.replaceState(null, "");
  navigation.groupId = null;
  openGroup.clear();
  groupList.all = [];
  session.account = account();
  dialogs.who = false;
  vi.mocked(api.getGroups).mockResolvedValue([]);
  vi.mocked(api.getAccount).mockResolvedValue(account());
  vi.spyOn(openGroup, "load").mockResolvedValue();
});

/** Opens the trip, as the app has it once loaded. */
function openTrip(balances = owing) {
  openGroup.group = group();
  openGroup.balances = balances;
  navigation.groupId = "g1";
}

describe("goHome", () => {
  it("leaves the open group and reloads the list", () => {
    openTrip();
    goHome();
    expect(navigation.groupId).toBeNull();
    expect(openGroup.group).toBeNull();
    expect(api.getGroups).toHaveBeenCalled();
  });
});

describe("createGroup", () => {
  it("opens the new group, whose first member the user is", async () => {
    vi.mocked(api.createGroup).mockResolvedValue(group({ id: "new" }));
    await createGroup("Trip", "EUR", ["Bob"]);
    expect(api.createGroup).toHaveBeenCalledWith("Trip", "EUR", ["Bob"]);
    expect(api.getAccount).toHaveBeenCalled();
    expect(navigation.groupId).toBe("new");
  });

  it("lets the dialog show what went wrong", async () => {
    vi.mocked(api.createGroup).mockRejectedValue(new Error("Group name cannot be empty"));
    await expect(createGroup("", "EUR", [])).rejects.toThrow("Group name cannot be empty");
    expect(navigation.groupId).toBeNull();
  });
});

describe("joinGroup", () => {
  it("opens the group joined and says so", async () => {
    vi.mocked(api.joinGroup).mockResolvedValue(group({ id: "joined", name: "Flat" }));
    await joinGroup("invite");
    expect(navigation.groupId).toBe("joined");
    expect(vi.mocked(toast.success).mock.calls[0][0]).toContain("Flat");
  });
});

describe("importGroup", () => {
  const csv = (name: string) => new File(["Date,Title"], name, { type: "text/csv" });

  it("names the group after the file", async () => {
    vi.mocked(api.importGroupCsv).mockResolvedValue(
      group({ id: "imported", expenses: [expense()] })
    );
    await importGroup(csv("Summer trip.CSV"));
    expect(api.importGroupCsv).toHaveBeenCalledWith("Summer trip", "Date,Title");
    expect(navigation.groupId).toBe("imported");
    expect(toast.success).toHaveBeenCalled();
  });

  it("gives a file without a name one", async () => {
    vi.mocked(api.importGroupCsv).mockResolvedValue(group());
    await importGroup(csv(".csv"));
    expect(vi.mocked(api.importGroupCsv).mock.calls[0][0]).not.toBe("");
  });

  it("says why a file couldn't be read", async () => {
    vi.mocked(api.importGroupCsv).mockRejectedValue(new Error("Not a CSV file"));
    await importGroup(csv("notes.csv"));
    expect(toast.error).toHaveBeenCalledWith(expect.any(String), { description: "Not a CSV file" });
    expect(navigation.groupId).toBeNull();
  });
});

describe("leaveGroup", () => {
  it("asks, then leaves and returns to the list", async () => {
    openTrip();
    const done = leaveGroup();
    const asked = await answerConfirm(true);
    await done;
    expect(asked.title).toContain("Trip");
    expect(api.leaveGroup).toHaveBeenCalledWith("g1");
    expect(navigation.groupId).toBeNull();
  });

  it("stays when the user says no, or when leaving fails", async () => {
    openTrip();
    let done = leaveGroup();
    await answerConfirm(false);
    await done;
    expect(api.leaveGroup).not.toHaveBeenCalled();

    vi.mocked(api.leaveGroup).mockRejectedValue(new Error("offline"));
    done = leaveGroup();
    await answerConfirm(true);
    await done;
    expect(navigation.groupId).toBe("g1");
    expect(toast.error).toHaveBeenCalled();
  });
});

describe("deleteGroup", () => {
  it("deletes a settled group for everyone once confirmed", async () => {
    openTrip([balance("alice", 0)]);
    vi.mocked(api.deleteGroup).mockResolvedValue(null);
    const done = deleteGroup();
    const asked = await answerConfirm(true);
    await done;
    expect(asked.confirmLabel).toBe(t("deletion.confirm"));
    expect(api.deleteGroup).toHaveBeenCalledWith("g1");
    expect(navigation.groupId).toBeNull();
    expect(toast.success).toHaveBeenCalled();
  });

  it("needs to know who the user is while someone owes something", async () => {
    openTrip();
    await deleteGroup();
    expect(dialogs.who).toBe(true);
    expect(toast.info).toHaveBeenCalled();
    expect(confirmation.open).toBe(false);
  });

  it("asks the others to agree while someone owes something", async () => {
    openTrip();
    session.account = account({ identities: { g1: "alice" } });
    vi.mocked(api.deleteGroup).mockResolvedValue(group({ deletion_votes: ["alice"] }));
    const done = deleteGroup();
    const asked = await answerConfirm(true);
    await done;
    expect(asked.confirmLabel).toBe(t("deletion.ask"));
    expect(openGroup.load).toHaveBeenCalledWith("g1");
    expect(navigation.groupId).toBe("g1");
    expect(toast.info).toHaveBeenCalled();
  });

  it("agrees to a deletion others asked for, the last agreement deleting", async () => {
    session.account = account({ identities: { g1: "alice" } });
    openTrip();
    openGroup.group = group({ deletion_votes: ["bob"] });
    let done = deleteGroup();
    expect((await answerConfirm(false)).confirmLabel).toBe(t("deletion.agree"));
    await done;

    openGroup.group = group({ deletion_votes: ["bob", "carol"] });
    done = deleteGroup();
    expect((await answerConfirm(false)).confirmLabel).toBe(t("deletion.confirm"));
    await done;
    expect(api.deleteGroup).not.toHaveBeenCalled();
  });
});

describe("refuseDeletion", () => {
  it("clears the agreements, or says why it couldn't", async () => {
    openTrip();
    const change = vi.spyOn(openGroup, "change").mockImplementation(async (apply) => {
      await apply("g1");
    });
    await refuseDeletion();
    expect(api.refuseGroupDeletion).toHaveBeenCalledWith("g1");
    change.mockRejectedValue(new Error("offline"));
    await refuseDeletion();
    expect(toast.error).toHaveBeenCalled();
  });
});

describe("setArchived", () => {
  it("puts the group away and returns to the list", async () => {
    openTrip();
    vi.mocked(api.setGroupArchived).mockResolvedValue(account({ archived: ["g1"] }));
    await setArchived(true);
    expect(session.isArchived("g1")).toBe(true);
    expect(navigation.groupId).toBeNull();
  });

  it("brings it back and stays in it", async () => {
    openTrip();
    vi.mocked(api.setGroupArchived).mockResolvedValue(account());
    await setArchived(false);
    expect(api.setGroupArchived).toHaveBeenCalledWith("g1", false);
    expect(navigation.groupId).toBe("g1");
  });
});
