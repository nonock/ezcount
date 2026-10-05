<script lang="ts">
  import { removeMember } from "@/lib/actions/members";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { session } from "@/lib/state/session.svelte";
  import { api } from "@/services/api";
  import AccountDialog from "./AccountDialog.svelte";
  import ActivityDialog from "./ActivityDialog.svelte";
  import AddExpenseModal from "./AddExpenseModal.svelte";
  import ChangePasswordDialog from "./ChangePasswordDialog.svelte";
  import CommentsDialog from "./CommentsDialog.svelte";
  import CreateGroupModal from "./CreateGroupModal.svelte";
  import DeleteAccountDialog from "./DeleteAccountDialog.svelte";
  import EditGroupModal from "./EditGroupModal.svelte";
  import ExpenseHistoryModal from "./ExpenseHistoryModal.svelte";
  import FeedbackDialog from "./FeedbackDialog.svelte";
  import JoinGroupModal from "./JoinGroupModal.svelte";
  import LinkDeviceDialog from "./LinkDeviceDialog.svelte";
  import MemberModal from "./MemberModal.svelte";
  import PayDialog from "./PayDialog.svelte";
  import RecordReimbursementModal from "./RecordReimbursementModal.svelte";
  import RecoveryKeyDialog from "./RecoveryKeyDialog.svelte";
  import RecurringDialog from "./RecurringDialog.svelte";
  import ShareGroupModal from "./ShareGroupModal.svelte";
  import TrashDialog from "./TrashDialog.svelte";
  import WhoAreYouModal from "./WhoAreYouModal.svelte";

  /**
   * The dialogs of someone logged in, each opened through `dialogs`: the account's, and the
   * open group's while there is one.
   */

  function addMember(name: string) {
    return openGroup.change((groupId) => api.addParticipant(groupId, name));
  }

  async function renameMember(name: string) {
    const member = dialogs.renameMember.member;
    if (member)
      await openGroup.change((groupId) => api.renameParticipant(groupId, member.id, name));
  }
</script>

<CreateGroupModal />
<JoinGroupModal />
<AccountDialog />
<ChangePasswordDialog />
<DeleteAccountDialog />
<LinkDeviceDialog />
<FeedbackDialog />
<RecoveryKeyDialog
  open={session.newRecoveryKey !== null}
  onClose={() => (session.newRecoveryKey = null)}
  reason={session.newRecoveryKey?.reason ?? "signup"}
  recoveryKey={session.newRecoveryKey?.key}
/>
<RecoveryKeyDialog
  open={dialogs.newRecoveryKey}
  onClose={() => (dialogs.newRecoveryKey = false)}
  reason="replace"
/>

{#if openGroup.group}
  {@const group = openGroup.group}
  <ShareGroupModal {group} />
  <WhoAreYouModal {group} />
  <EditGroupModal {group} />
  <ActivityDialog {group} />
  <TrashDialog {group} />
  <RecurringDialog {group} />
  <AddExpenseModal {group} />
  <RecordReimbursementModal {group} />
  <ExpenseHistoryModal {group} />
  <CommentsDialog {group} />
  <PayDialog {group} />
  <MemberModal bind:open={dialogs.addMember} onSubmit={addMember} />
  <MemberModal
    bind:open={dialogs.renameMember.open}
    member={dialogs.renameMember.member}
    onSubmit={renameMember}
    onRemove={() => {
      const member = dialogs.renameMember.member;
      if (member) removeMember(member.id);
    }}
  />
{/if}
