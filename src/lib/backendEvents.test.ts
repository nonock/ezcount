import { toast } from "svelte-sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/services/api";
import { account, group } from "@/test/fixtures";
import type { SyncInfo } from "@/types";
import { accountUpdated, syncUpdated } from "./backendEvents.svelte";
import { groupList, openGroup } from "./state/groups.svelte";
import { navigation } from "./state/navigation.svelte";
import { session } from "./state/session.svelte";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@/services/api", () => ({
  api: { getGroups: vi.fn(), getSyncInfo: vi.fn(), getAccount: vi.fn() },
}));
vi.mock("@/services/native.svelte", () => ({ closeTopLayer: () => false }));
vi.mock("svelte-sonner", () => ({ toast: { info: vi.fn() } }));

const synced: SyncInfo = {
  group_id: "g1",
  enabled: true,
  server_url: "https://relay.example.com",
  invite_code: null,
  last_synced_at: "2026-03-01T10:00:00Z",
  last_error: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  window.history.replaceState(null, "");
  navigation.groupId = "g1";
  openGroup.clear();
  groupList.all = [];
  vi.mocked(api.getGroups).mockResolvedValue([group()]);
  vi.mocked(api.getSyncInfo).mockResolvedValue(synced);
  vi.mocked(api.getAccount).mockResolvedValue(account());
});

describe("syncUpdated", () => {
  it("offers what changed in the open group rather than applying it", async () => {
    syncUpdated({ group_id: "g1", changed: true });
    expect(openGroup.stale).toBe(true);
    expect(api.getGroups).toHaveBeenCalled();
    await vi.waitFor(() => expect(openGroup.syncInfo).toEqual(synced));
  });

  it("only updates the sync state when nothing changed", async () => {
    syncUpdated({ group_id: "g1", changed: false });
    expect(openGroup.stale).toBe(false);
    expect(api.getGroups).not.toHaveBeenCalled();
    await vi.waitFor(() => expect(openGroup.syncInfo).toEqual(synced));
  });

  it("refreshes the list for a change in a group that isn't open", () => {
    syncUpdated({ group_id: "g2", changed: true });
    expect(api.getGroups).toHaveBeenCalled();
    expect(openGroup.stale).toBe(false);
    expect(api.getSyncInfo).not.toHaveBeenCalled();
  });

  it("goes on when the sync state can't be read", async () => {
    vi.mocked(api.getSyncInfo).mockRejectedValue(new Error("Group not found"));
    syncUpdated({ group_id: "g1", changed: false });
    await Promise.resolve();
    expect(openGroup.syncInfo).toBeNull();
  });
});

describe("accountUpdated", () => {
  it("takes the account and the groups as another device left them", async () => {
    vi.mocked(api.getAccount).mockResolvedValue(account({ display_name: "Alice M." }));
    await accountUpdated();
    expect(session.name).toBe("Alice M.");
    expect(groupList.all).toHaveLength(1);
    expect(navigation.groupId).toBe("g1");
    expect(toast.info).not.toHaveBeenCalled();
  });

  it("leaves the open group when another device removed it", async () => {
    vi.mocked(api.getGroups).mockResolvedValue([group({ id: "g2" })]);
    await accountUpdated();
    expect(navigation.groupId).toBeNull();
    expect(toast.info).toHaveBeenCalled();
  });
});
