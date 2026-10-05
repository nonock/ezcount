// Leaving the account: logging out of it, or deleting it.

import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { errorMessage } from "@/utils/errors";
import { t } from "../i18n/index.svelte";
import { askConfirm } from "../state/confirm.svelte";
import { dialogs } from "../state/dialogs.svelte";
import { groupList, openGroup } from "../state/groups.svelte";
import { navigation } from "../state/navigation.svelte";
import { session } from "../state/session.svelte";

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
  leaveSession();
}

/**
 * Deletes the account for good, which takes its password, and goes back to the login screen.
 * Throws what the core refuses (a wrong password, a server out of reach).
 */
export async function deleteAccount(password: string) {
  await api.deleteAccount(password);
  leaveSession();
  toast.success(t("deleteAccount.done"));
}

/** Forgets the account and its groups, once the core has: the login screen is next. */
export function leaveSession() {
  navigation.close();
  openGroup.clear();
  groupList.all = [];
  dialogs.reset();
  session.account = null;
}
