// What the user does that crosses screens or asks for confirmation first. Simple changes to
// the open group go through `openGroup.change` from the components instead.

import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { isAndroid, nativeFeatures, ScanCancelled, scanQrCode } from "@/services/native.svelte";
import { errorMessage } from "@/utils/errors";
import { expenseTitle } from "@/utils/formatters";
import { t } from "./i18n/index.svelte";
import { askConfirm } from "./state/confirm.svelte";
import { dialogs } from "./state/dialogs.svelte";
import { groupList, openGroup } from "./state/groups.svelte";
import { navigation } from "./state/navigation.svelte";
import { session } from "./state/session.svelte";

export function goHome() {
  navigation.close();
  openGroup.clear();
  groupList.refresh();
}

export async function logOut() {
  const account = session.account;
  if (!account) return;
  const confirmed = await askConfirm({
    title: t("logout.title", account.username),
    description: t("logout.description"),
    confirmLabel: t("logout.confirm"),
  });
  if (!confirmed) return;
  try {
    await api.logOut(false);
  } catch (err) {
    const force = await askConfirm({
      title: t("logout.anyway"),
      description: errorMessage(err),
      confirmLabel: t("logout.confirmAnyway"),
      destructive: true,
    });
    if (!force) return;
    try {
      await api.logOut(true);
    } catch (forceErr) {
      toast.error(t("logout.failed"), { description: errorMessage(forceErr) });
      return;
    }
  }
  navigation.close();
  openGroup.clear();
  groupList.all = [];
  dialogs.identitySkipped.clear();
  session.account = null;
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

/**
 * Saves the open group as a CSV file and says where it went: the Downloads folder on a
 * computer, the browser's downloads in the web version. On Android it goes to the share sheet.
 */
export async function exportGroup() {
  const group = openGroup.group;
  if (!group) return;
  try {
    const csv = await api.exportGroupCsv(group.id);
    const fileName = `${group.name.replace(/[/:*?"<>|]/g, "_")}.csv`;
    if (isAndroid) {
      // Android's web view can't download what the page makes.
      await api.shareText(csv, fileName);
      return;
    }
    // The byte order mark makes Excel read the accents right.
    const text = `\uFEFF${csv}`;
    if (nativeFeatures.save) {
      const path = await api.saveDownload(fileName, text);
      toast.success(t("groups.exported"), {
        description: path,
        duration: 10_000,
        action: { label: t("groups.showFile"), onClick: () => revealItemInDir(path) },
      });
      return;
    }
    const url = URL.createObjectURL(new Blob([text], { type: "text/csv" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = fileName;
    link.click();
    URL.revokeObjectURL(url);
    toast.success(t("groups.exported"), { description: t("groups.exportedBrowser", fileName) });
  } catch (err) {
    toast.error(t("groups.exportFailed"), { description: errorMessage(err) });
  }
}

/**
 * Scanning is a deliberate act, so a scanned invite is joined right away. Problems, or
 * cancelling, lead back to the join dialog.
 */
export async function scanInvite() {
  dialogs.join.open = false;
  dialogs.scanning = true;
  let code: string;
  try {
    code = await scanQrCode();
  } catch (err) {
    dialogs.scanning = false;
    dialogs.openJoin("", err instanceof ScanCancelled ? null : errorMessage(err));
    return;
  }
  dialogs.scanning = false;
  try {
    await joinGroup(code);
  } catch (err) {
    dialogs.openJoin(code, errorMessage(err));
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

export async function removeMember(participantId: string) {
  const participant = openGroup.group?.participants.find((p) => p.id === participantId);
  if (!participant) return;
  const confirmed = await askConfirm({
    title: t("group.removeMemberTitle", participant.name),
    description: t("group.removeMemberDescription"),
    confirmLabel: t("common.remove"),
    destructive: true,
  });
  if (!confirmed) return;
  try {
    await openGroup.change((groupId) => api.removeParticipant(groupId, participantId));
  } catch (err) {
    toast.error(t("group.removeMemberFailed"), { description: errorMessage(err) });
  }
}

/** Moves an expense to the group's trash. It isn't asked first: it can be undone. */
export async function deleteExpense(expenseId: string) {
  const expense = openGroup.group?.expenses.find((e) => e.id === expenseId);
  if (!expense) return;
  try {
    await openGroup.change((groupId) => api.deleteExpense(groupId, expenseId));
    toast.success(t("trash.moved", expenseTitle(expense)), {
      duration: 8000,
      action: { label: t("trash.undo"), onClick: () => restoreExpense(expenseId) },
    });
  } catch (err) {
    toast.error(t("expenses.deleteFailed"), { description: errorMessage(err) });
  }
}

/** Puts a deleted expense back among the others. */
export async function restoreExpense(expenseId: string) {
  try {
    await openGroup.change((groupId) => api.restoreExpense(groupId, expenseId));
  } catch (err) {
    toast.error(t("trash.restoreFailed"), { description: errorMessage(err) });
  }
}

/** Removes a deleted expense from the trash, for every member and for good. */
export async function purgeExpense(expenseId: string) {
  const deleted = openGroup.group?.trash?.find((d) => d.expense.id === expenseId);
  if (!deleted) return;
  const confirmed = await askConfirm({
    title: t("trash.purgeTitle", expenseTitle(deleted.expense)),
    description: t("trash.purgeDescription"),
    confirmLabel: t("trash.purgeConfirm"),
    destructive: true,
  });
  if (!confirmed) return;
  try {
    await openGroup.change((groupId) => api.purgeExpense(groupId, expenseId));
  } catch (err) {
    toast.error(t("expenses.deleteFailed"), { description: errorMessage(err) });
  }
}

/** Stops an expense from coming back. The ones already added stay. */
export async function stopRecurring(recurringId: string) {
  const model = openGroup.group?.recurring?.find((r) => r.id === recurringId);
  if (!model) return;
  const confirmed = await askConfirm({
    title: t("recurring.stopTitle", model.title),
    description: t("recurring.stopDescription"),
    confirmLabel: t("recurring.stop"),
  });
  if (!confirmed) return;
  try {
    await openGroup.change((groupId) => api.stopRecurringExpense(groupId, recurringId));
  } catch (err) {
    toast.error(t("recurring.stopFailed"), { description: errorMessage(err) });
  }
}

export async function chooseIdentity(participantId: string) {
  const group = openGroup.group;
  if (!group) return;
  session.account = await api.setIdentity(group.id, participantId);
  // That member now carries the profile's name and picture.
  await openGroup.load(group.id);
  await groupList.refresh();
}

export async function addSelf(name: string) {
  const group = openGroup.group;
  if (!group) return;
  await openGroup.change((groupId) => api.addSelf(groupId, name));
  await session.refresh();
}

/** Closing "Who are you?" without answering: don't ask again in this group this session. */
export function skipIdentity() {
  dialogs.who = false;
  const group = openGroup.group;
  if (group && !openGroup.currentUserId) dialogs.identitySkipped.add(group.id);
}
