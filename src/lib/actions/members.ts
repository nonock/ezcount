// Who is in the open group, and which of them the user is.

import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { errorMessage } from "@/utils/errors";
import { t } from "../i18n/index.svelte";
import { askConfirm } from "../state/confirm.svelte";
import { dialogs } from "../state/dialogs.svelte";
import { groupList, openGroup } from "../state/groups.svelte";
import { session } from "../state/session.svelte";

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
