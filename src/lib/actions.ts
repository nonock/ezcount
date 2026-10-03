// What the user does that crosses screens or asks for confirmation first. Simple changes to
// the open group go through `openGroup.change` from the components instead.

import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { isAndroid, ScanCancelled, scanQrCode } from "@/services/native.svelte";
import { errorMessage } from "@/utils/errors";
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
    title: `Log out of ${account.username}?`,
    description:
      "Your groups are removed from this device. They stay in your account: log in again to get them back.",
    confirmLabel: "Log Out",
  });
  if (!confirmed) return;
  try {
    await api.logOut(false);
  } catch (err) {
    const force = await askConfirm({
      title: "Log out anyway?",
      description: errorMessage(err),
      confirmLabel: "Log Out Anyway",
      destructive: true,
    });
    if (!force) return;
    try {
      await api.logOut(true);
    } catch (forceErr) {
      toast.error("Could not log out", { description: errorMessage(forceErr) });
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
  toast.success(`Joined "${group.name}"`);
}

/** Creates a group from a CSV file, named after the file. The app then asks who the user is. */
export async function importGroup(file: File) {
  try {
    const name = file.name.replace(/.csv$/i, "").trim() || "Imported group";
    const group = await api.importGroupCsv(name, await file.text());
    await groupList.refresh();
    navigation.open(group.id);
    toast.success(`Imported "${group.name}"`, {
      description: `${group.expenses.length} records, ${group.participants.length} people`,
    });
  } catch (err) {
    toast.error("Could not import the file", { description: errorMessage(err) });
  }
}

/** Saves the open group as a CSV file; on Android, hands it to the share sheet. */
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
    const url = URL.createObjectURL(new Blob(["﻿", csv], { type: "text/csv" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = fileName;
    link.click();
    URL.revokeObjectURL(url);
  } catch (err) {
    toast.error("Could not export the group", { description: errorMessage(err) });
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
    title: `Leave "${group.name}"?`,
    description:
      "It's removed from your account on all your devices. Other members keep the group, and you can rejoin with an invite link.",
    confirmLabel: "Leave Group",
    destructive: true,
  });
  if (!confirmed) return;
  try {
    await api.leaveGroup(group.id);
    goHome();
  } catch (err) {
    toast.error("Could not leave the group", { description: errorMessage(err) });
  }
}

export async function removeMember(participantId: string) {
  const participant = openGroup.group?.participants.find((p) => p.id === participantId);
  if (!participant) return;
  const confirmed = await askConfirm({
    title: `Remove ${participant.name}?`,
    description:
      "Their past expenses and balance are kept, but they can't be added to new expenses.",
    confirmLabel: "Remove",
    destructive: true,
  });
  if (!confirmed) return;
  try {
    await openGroup.change((groupId) => api.removeParticipant(groupId, participantId));
  } catch (err) {
    toast.error("Could not remove the member", { description: errorMessage(err) });
  }
}

export async function deleteExpense(expenseId: string) {
  const expense = openGroup.group?.expenses.find((e) => e.id === expenseId);
  const confirmed = await askConfirm({
    title: `Delete "${expense?.title ?? "this record"}"?`,
    description: "Balances are recalculated without it. This can't be undone.",
    confirmLabel: "Delete",
    destructive: true,
  });
  if (!confirmed) return;
  try {
    await openGroup.change((groupId) => api.deleteExpense(groupId, expenseId));
  } catch (err) {
    toast.error("Could not delete the record", { description: errorMessage(err) });
  }
}

export async function chooseIdentity(participantId: string) {
  const group = openGroup.group;
  if (!group) return;
  session.account = await api.setIdentity(group.id, participantId);
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
