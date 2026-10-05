// What starts with the camera: joining a group, or handing another device an invite or the
// account.

import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { ScanCancelled, scanQrCode } from "@/services/native.svelte";
import { errorMessage } from "@/utils/errors";
import { t } from "../i18n/index.svelte";
import { dialogs } from "../state/dialogs.svelte";
import { openGroup } from "../state/groups.svelte";
import { joinGroup } from "./groups";

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

/** The code a device shows to get something from this phone (see `ReceiveCode.svelte`). */
async function scanReceiveCode(): Promise<string | null> {
  dialogs.scanning = true;
  try {
    return await scanQrCode();
  } catch (err) {
    if (!(err instanceof ScanCancelled)) toast.error(errorMessage(err));
    return null;
  } finally {
    dialogs.scanning = false;
  }
}

/** Scans the code a computer shows to join a group, and sends it the open group's invite. */
export async function sendInviteToScanned() {
  const group = openGroup.group;
  if (!group) return;
  dialogs.share = false;
  const link = await scanReceiveCode();
  if (!link) return;
  try {
    await api.sendGroupInvite(group.id, link);
    toast.success(t("share.sent", group.name));
  } catch (err) {
    toast.error(t("share.sendFailed"), { description: errorMessage(err) });
  }
}

/** Scans the code a device shows on its login screen, and logs it into this account. */
export async function sendLoginToScanned(password: string) {
  dialogs.linkDevice = false;
  const link = await scanReceiveCode();
  if (!link) return;
  try {
    await api.sendLogin(link, password);
    toast.success(t("link.sent"));
  } catch (err) {
    toast.error(t("link.sendFailed"), { description: errorMessage(err) });
  }
}
