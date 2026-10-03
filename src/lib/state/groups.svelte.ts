// The groups on this device, and the open one with its balances and sync state.

import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import type { Group, ParticipantBalance, SettlementTransfer, SyncInfo } from "@/types";
import { errorMessage } from "@/utils/errors";
import { t } from "../i18n/index.svelte";
import { navigation } from "./navigation.svelte";
import { session } from "./session.svelte";

class GroupList {
  all = $state<Group[]>([]);
  loading = $state(true);

  async refresh() {
    try {
      this.all = await api.getGroups();
    } catch (err) {
      console.error("Failed to load groups:", err);
    } finally {
      this.loading = false;
    }
  }
}

export const groupList = new GroupList();

class OpenGroup {
  group = $state<Group | null>(null);
  balances = $state<ParticipantBalance[]>([]);
  settlements = $state<SettlementTransfer[]>([]);
  syncInfo = $state<SyncInfo | null>(null);
  /**
   * Other devices changed the group since it was loaded. It isn't reloaded under the user's
   * eyes, which would move what they are reading: they are offered to.
   */
  stale = $state(false);

  /** The participant the user is in this group, if they said. */
  get currentUserId(): string | null {
    return this.group ? session.identityIn(this.group.id) : null;
  }

  async load(groupId: string) {
    try {
      const group = await api.getGroup(groupId);
      const [balances, settlements, syncInfo] = await Promise.all([
        api.getBalances(groupId),
        api.getSettlements(groupId),
        api.getSyncInfo(groupId),
      ]);
      // The user went elsewhere meanwhile.
      if (navigation.groupId !== groupId) return;
      this.stale = false;
      this.group = group;
      this.balances = balances;
      this.settlements = settlements;
      this.syncInfo = syncInfo;
    } catch (err) {
      console.error("Failed to load active group:", err);
      toast.error(t("groups.openFailed"), { description: errorMessage(err) });
      navigation.close();
      this.clear();
    }
  }

  clear() {
    this.stale = false;
    this.group = null;
    this.balances = [];
    this.settlements = [];
    this.syncInfo = null;
  }

  /** Applies a change to the open group, then reloads it and the group list. Throws its error. */
  async change(apply: (groupId: string) => Promise<Group>) {
    if (!this.group) return;
    const updated = await apply(this.group.id);
    this.group = updated;
    await this.load(updated.id);
    await groupList.refresh();
  }

  async syncNow() {
    const groupId = navigation.groupId;
    if (!groupId) return;
    this.syncInfo = await api.syncNow(groupId);
    await this.load(groupId);
    await groupList.refresh();
  }
}

export const openGroup = new OpenGroup();
