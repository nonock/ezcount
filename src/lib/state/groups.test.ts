import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { account, group } from "@/test/fixtures";
import type { SyncInfo } from "@/types";
import { groupList, openGroup } from "./groups.svelte";
import { navigation } from "./navigation.svelte";
import { session } from "./session.svelte";

vi.mock("@/services/api", () => ({
  api: {
    getGroups: vi.fn(),
    getGroup: vi.fn(),
    getBalances: vi.fn(),
    getSettlements: vi.fn(),
    getSyncInfo: vi.fn(),
    syncNow: vi.fn(),
  },
}));
vi.mock("@/services/native.svelte", () => ({ closeTopLayer: () => false }));
vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn() } }));

const syncInfo: SyncInfo = {
  group_id: "g1",
  enabled: true,
  server_url: "https://relay.example.com",
  invite_code: null,
  last_synced_at: null,
  last_error: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  window.history.replaceState(null, "");
  navigation.groupId = null;
  openGroup.clear();
  groupList.all = [];
  groupList.loading = true;
  vi.mocked(api.getGroups).mockResolvedValue([group()]);
  vi.mocked(api.getGroup).mockResolvedValue(group());
  vi.mocked(api.getBalances).mockResolvedValue([]);
  vi.mocked(api.getSettlements).mockResolvedValue([]);
  vi.mocked(api.getSyncInfo).mockResolvedValue(syncInfo);
});

describe("groupList", () => {
  it("loads the groups", async () => {
    await groupList.refresh();
    expect(groupList.all).toHaveLength(1);
    expect(groupList.loading).toBe(false);
  });

  it("keeps the groups it had when loading fails", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    groupList.all = [group({ id: "old" })];
    vi.mocked(api.getGroups).mockRejectedValue(new Error("Database is locked"));
    await groupList.refresh();
    expect(groupList.all[0].id).toBe("old");
    expect(groupList.loading).toBe(false);
  });
});

describe("openGroup", () => {
  it("loads the open group with its balances and sync state", async () => {
    navigation.groupId = "g1";
    openGroup.stale = true;
    await openGroup.load("g1");
    expect(openGroup.group?.id).toBe("g1");
    expect(openGroup.syncInfo).toEqual(syncInfo);
    expect(openGroup.stale).toBe(false);
  });

  it("drops what arrives once the user went elsewhere", async () => {
    navigation.groupId = "g2";
    await openGroup.load("g1");
    expect(openGroup.group).toBeNull();
  });

  it("says so and returns to the list when the group can't be opened", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    navigation.groupId = "g1";
    vi.mocked(api.getGroup).mockRejectedValue(new Error("Group not found"));
    await openGroup.load("g1");
    expect(toast.error).toHaveBeenCalledWith(expect.any(String), {
      description: "Group not found",
    });
    expect(navigation.groupId).toBeNull();
    expect(openGroup.group).toBeNull();
  });

  it("knows the participant the user is in the open group", async () => {
    expect(openGroup.currentUserId).toBeNull();
    navigation.groupId = "g1";
    await openGroup.load("g1");
    session.account = account({ identities: { g1: "bob" } });
    expect(openGroup.currentUserId).toBe("bob");
  });

  it("applies a change, then reloads the group and the list", async () => {
    navigation.groupId = "g1";
    await openGroup.load("g1");
    const renamed = group({ name: "Holidays" });
    vi.mocked(api.getGroup).mockResolvedValue(renamed);
    const apply = vi.fn().mockResolvedValue(renamed);
    await openGroup.change(apply);
    expect(apply).toHaveBeenCalledWith("g1");
    expect(openGroup.group?.name).toBe("Holidays");
    expect(api.getGroups).toHaveBeenCalled();
  });

  it("changes nothing without an open group, and lets a change's error through", async () => {
    const apply = vi.fn().mockRejectedValue(new Error("Expense not found"));
    await openGroup.change(apply);
    expect(apply).not.toHaveBeenCalled();
    navigation.groupId = "g1";
    await openGroup.load("g1");
    await expect(openGroup.change(apply)).rejects.toThrow("Expense not found");
  });

  it("syncs the open group and reloads it", async () => {
    await openGroup.syncNow();
    expect(api.syncNow).not.toHaveBeenCalled();
    navigation.groupId = "g1";
    vi.mocked(api.syncNow).mockResolvedValue({ ...syncInfo, last_synced_at: "2026-03-01" });
    await openGroup.syncNow();
    expect(api.syncNow).toHaveBeenCalledWith("g1");
    expect(openGroup.group?.id).toBe("g1");
  });
});
