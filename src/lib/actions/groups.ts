// What the user does with whole groups: making, joining, leaving and deleting them.

import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { errorMessage } from "@/utils/errors";
import { t } from "../i18n/index.svelte";
import { askConfirm } from "../state/confirm.svelte";
import { dialogs } from "../state/dialogs.svelte";
import { groupList, openGroup } from "../state/groups.svelte";
import { navigation } from "../state/navigation.svelte";
import { session } from "../state/session.svelte";

export function goHome() {
  navigation.close();
  openGroup.clear();
  groupList.refresh();
}

/** Errors propagate to the dialog, which shows them inline. */
export async function createGroup(name: string, currency: string, participants: string[]) {
  const group = await api.createGroup(name, currency, participants);
  // The backend recorded the creator as the first participant.
  await session.refresh();
  await groupList.refresh();
  navigation.open(group.id);
}

/** Errors propagate to the dialog, which shows them inline. */
export async function joinGroup(inviteCode: string) {
  const group = await api.joinGroup(inviteCode);
  await groupList.refresh();
  navigation.open(group.id);
  toast.success(t("groups.joined", group.name));
}

/** Creates a group from a CSV file, named after the file. The app then asks who the user is. */
export async function importGroup(file: File) {
  try {
    const name = file.name.replace(/.csv$/i, "").trim() || t("groups.importedName");
    const group = await api.importGroupCsv(name, await file.text());
    await groupList.refresh();
    navigation.open(group.id);
    toast.success(t("groups.imported", group.name), {
      description: t("groups.importedDetail", group.expenses.length, group.participants.length),
    });
  } catch (err) {
    toast.error(t("groups.importFailed"), { description: errorMessage(err) });
  }
}

export async function leaveGroup() {
  const group = openGroup.group;
  if (!group) return;
  const confirmed = await askConfirm({
    title: t("group.leaveTitle", group.name),
    description: t("group.leaveDescription"),
    confirmLabel: t("group.leaveConfirm"),
    destructive: true,
  });
  if (!confirmed) return;
  try {
    await api.leaveGroup(group.id);
    goHome();
  } catch (err) {
    toast.error(t("group.leaveFailed"), { description: errorMessage(err) });
  }
}

/**
 * Deletes the open group for everyone. While someone still owes something, it takes every
 * member's agreement: this gives the user's, and the group waits for the others.
 */
export async function deleteGroup() {
  const group = openGroup.group;
  if (!group) return;
  const me = openGroup.currentUserId;
  const settled = openGroup.balances.every((b) => b.net_cents === 0);
  if (!settled && !me) {
    toast.info(t("deletion.needsIdentity"));
    dialogs.who = true;
    return;
  }
  const votes = group.deletion_votes ?? [];
  const others = group.participants.filter((p) => !p.removed && p.id !== me);
  // The last agreement deletes, like settled balances do.
  const deletes = settled || others.every((p) => votes.includes(p.id));
  const confirmed = await askConfirm(
    deletes
      ? {
          title: t("deletion.title", group.name),
          description: t("deletion.description"),
          confirmLabel: t("deletion.confirm"),
          destructive: true,
        }
      : {
          title: t("deletion.askTitle", group.name),
          description: t("deletion.askDescription"),
          confirmLabel: votes.length > 0 ? t("deletion.agree") : t("deletion.ask"),
          destructive: true,
        }
  );
  if (!confirmed) return;
  try {
    const waiting = await api.deleteGroup(group.id);
    if (waiting) {
      await openGroup.load(group.id);
      toast.info(t("deletion.waitingToast"));
    } else {
      toast.success(t("deletion.done", group.name));
      goHome();
    }
  } catch (err) {
    toast.error(t("deletion.failed"), { description: errorMessage(err) });
  }
}

/** Refuses the deletion other members asked for, or takes the user's own request back. */
export async function refuseDeletion() {
  try {
    await openGroup.change((groupId) => api.refuseGroupDeletion(groupId));
  } catch (err) {
    toast.error(t("deletion.failed"), { description: errorMessage(err) });
  }
}

/** Puts the open group away for the user, or back among the others. */
export async function setArchived(archived: boolean) {
  const group = openGroup.group;
  if (!group) return;
  try {
    session.account = await api.setGroupArchived(group.id, archived);
    toast.success(t(archived ? "archive.done" : "archive.undone", group.name));
    if (archived) goHome();
  } catch (err) {
    toast.error(t("archive.failed"), { description: errorMessage(err) });
  }
}
