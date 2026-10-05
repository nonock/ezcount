// Leaving the account.

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
  navigation.close();
  openGroup.clear();
  groupList.all = [];
  dialogs.identitySkipped.clear();
  session.account = null;
}
