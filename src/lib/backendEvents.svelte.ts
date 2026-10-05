// What the app does when its core says something happened in the background: a sync pass
// ended, or another device changed the account.

import { listen } from "@tauri-apps/api/event";
import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { t } from "./i18n/index.svelte";
import { groupList, openGroup } from "./state/groups.svelte";
import { navigation } from "./state/navigation.svelte";
import { session } from "./state/session.svelte";

interface SyncUpdatedEvent {
  group_id: string;
  changed: boolean;
}

/** Subscribes to a backend event for as long as the component calling this lives. */
function onEvent<T>(event: string, handler: (payload: T) => void) {
  $effect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    listen<T>(event, ({ payload }) => handler(payload))
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((err) => console.error(`Could not subscribe to ${event}:`, err));
    return () => {
      disposed = true;
      unlisten?.();
    };
  });
}

/**
 * Background sync reports every attempt. What another device changed in the open group is
 * offered rather than applied, so the list doesn't move while it is being read.
 */
export function syncUpdated(payload: SyncUpdatedEvent) {
  if (payload.changed) groupList.refresh();
  if (payload.group_id !== navigation.groupId) return;
  if (payload.changed) openGroup.stale = true;
  api
    .getSyncInfo(payload.group_id)
    .then((info) => {
      openGroup.syncInfo = info;
    })
    .catch(() => {});
}

/** Another device of this account joined or left a group, or changed who the user is. */
export async function accountUpdated() {
  await session.refresh();
  const list = await api.getGroups();
  groupList.all = list;
  const selected = navigation.groupId;
  if (selected && !list.some((g) => g.id === selected)) {
    navigation.close();
    toast.info(t("app.removedElsewhere"));
  }
}

/** Follows the core's events. Call it while a component initializes. */
export function followBackend() {
  onEvent<SyncUpdatedEvent>("sync-updated", syncUpdated);
  onEvent("account-updated", accountUpdated);
}
