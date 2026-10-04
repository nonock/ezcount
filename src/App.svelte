<script lang="ts">
  import PlusIcon from "@lucide/svelte/icons/plus";
  import TriangleAlertIcon from "@lucide/svelte/icons/triangle-alert";
  import { listen } from "@tauri-apps/api/event";
  import { ModeWatcher } from "mode-watcher";
  import { untrack } from "svelte";
  import { toast } from "svelte-sonner";
  import AuthScreen from "@/components/auth/AuthScreen.svelte";
  import BottomBar from "@/components/common/BottomBar.svelte";
  import ConfirmDialog from "@/components/common/ConfirmDialog.svelte";
  import Navbar from "@/components/common/Navbar.svelte";
  import QrScanOverlay from "@/components/common/QrScanOverlay.svelte";
  import Splash from "@/components/common/Splash.svelte";
  import GroupDashboard from "@/components/dashboard/GroupDashboard.svelte";
  import AccountDialog from "@/components/modals/AccountDialog.svelte";
  import ActivityDialog from "@/components/modals/ActivityDialog.svelte";
  import AddExpenseModal from "@/components/modals/AddExpenseModal.svelte";
  import ChangePasswordDialog from "@/components/modals/ChangePasswordDialog.svelte";
  import CommentsDialog from "@/components/modals/CommentsDialog.svelte";
  import CreateGroupModal from "@/components/modals/CreateGroupModal.svelte";
  import EditGroupModal from "@/components/modals/EditGroupModal.svelte";
  import ExpenseHistoryModal from "@/components/modals/ExpenseHistoryModal.svelte";
  import FeedbackDialog from "@/components/modals/FeedbackDialog.svelte";
  import JoinGroupModal from "@/components/modals/JoinGroupModal.svelte";
  import LinkDeviceDialog from "@/components/modals/LinkDeviceDialog.svelte";
  import MemberModal from "@/components/modals/MemberModal.svelte";
  import PayDialog from "@/components/modals/PayDialog.svelte";
  import RecordReimbursementModal from "@/components/modals/RecordReimbursementModal.svelte";
  import RecoveryKeyDialog from "@/components/modals/RecoveryKeyDialog.svelte";
  import RecurringDialog from "@/components/modals/RecurringDialog.svelte";
  import ShareGroupModal from "@/components/modals/ShareGroupModal.svelte";
  import TrashDialog from "@/components/modals/TrashDialog.svelte";
  import WhoAreYouModal from "@/components/modals/WhoAreYouModal.svelte";
  import * as Alert from "@/components/ui/alert";
  import { Button } from "@/components/ui/button";
  import { Spinner } from "@/components/ui/spinner";
  import GroupPage from "@/components/workspace/GroupPage.svelte";
  import { goHome, removeMember } from "@/lib/actions";
  import { backendText } from "@/lib/i18n/backend";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { groupList, openGroup } from "@/lib/state/groups.svelte";
  import { navigation } from "@/lib/state/navigation.svelte";
  import { type NewRecoveryKey, session } from "@/lib/state/session.svelte";
  import { api } from "@/services/api";
  import {
    cancelScan,
    handleAndroidBack,
    loadNativeFeatures,
    onInviteLink,
  } from "@/services/native.svelte";
  import type { AccountInfo } from "@/types";

  interface SyncUpdatedEvent {
    group_id: string;
    changed: boolean;
  }

  let storageWarnings = $state<string[]>([]);
  // An invite link the app was opened with, kept until the user is logged in.
  let pendingInvite = $state<string | null>(null);

  session.refresh();
  loadNativeFeatures();
  api
    .getStorageWarnings()
    .then((warnings) => (storageWarnings = warnings))
    .catch((err) => console.error("Failed to load storage warnings:", err));

  $effect(() => onInviteLink((link) => (pendingInvite = link)));
  $effect(() => navigation.listen());

  $effect(() => {
    if (!session.loggedIn || !pendingInvite) return;
    const code = pendingInvite;
    untrack(() => {
      dialogs.openJoin(code);
      pendingInvite = null;
    });
  });

  $effect(() => {
    if (session.loggedIn) untrack(() => groupList.refresh());
  });

  // The open group follows the navigation, including Back and Forward.
  $effect(() => {
    const groupId = navigation.groupId;
    untrack(() => (groupId ? openGroup.load(groupId) : openGroup.clear()));
  });

  /** Subscribes to a backend event for as long as the app runs. */
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

  // Background sync reports every attempt. What another device changed in the open group is
  // offered rather than applied, so the list doesn't move while it is being read.
  onEvent<SyncUpdatedEvent>("sync-updated", (payload) => {
    if (payload.changed) groupList.refresh();
    if (payload.group_id !== navigation.groupId) return;
    if (payload.changed) openGroup.stale = true;
    api
      .getSyncInfo(payload.group_id)
      .then((info) => (openGroup.syncInfo = info))
      .catch(() => {});
  });

  // Another device of this account joined or left a group, or changed who the user is.
  onEvent("account-updated", async () => {
    await session.refresh();
    const list = await api.getGroups();
    groupList.all = list;
    const selected = navigation.groupId;
    if (selected && !list.some((g) => g.id === selected)) {
      navigation.close();
      toast.info(t("app.removedElsewhere"));
    }
  });

  // Catch up as soon as the app comes back to the foreground (e.g. reopening it on a phone).
  const isShared = $derived(Boolean(openGroup.syncInfo?.enabled));
  $effect(() => {
    if (!isShared) return;
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        openGroup.syncNow().catch((err) => console.error("Sync failed:", err));
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  // Ask once per group who the user is; the answer is kept in their account.
  const needsIdentity = $derived(
    Boolean(
      openGroup.group &&
      session.account &&
      !openGroup.currentUserId &&
      !dialogs.identitySkipped.has(openGroup.group.id)
    )
  );
  $effect(() => {
    if (needsIdentity) untrack(() => (dialogs.who = true));
  });

  // While a dialog is open, the app behind it is hidden from screen readers. Dialogs render
  // outside the app's root, at the end of the page.
  $effect(() => {
    const root = document.getElementById("root");
    if (!root) return;
    const update = () => {
      const modal = document.querySelector('[aria-modal="true"][data-state="open"]');
      if (modal && !root.contains(modal)) root.setAttribute("aria-hidden", "true");
      else root.removeAttribute("aria-hidden");
    };
    const observer = new MutationObserver(update);
    observer.observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ["data-state"],
    });
    update();
    return () => {
      observer.disconnect();
      root.removeAttribute("aria-hidden");
    };
  });

  handleAndroidBack(() => (dialogs.scanning ? stopScanning : openGroup.group ? goHome : null));

  function stopScanning() {
    cancelScan().catch((err) => console.error("Could not stop scanning:", err));
  }

  function onAuthenticated(account: AccountInfo, recoveryKey?: NewRecoveryKey) {
    groupList.loading = true;
    session.account = account;
    session.newRecoveryKey = recoveryKey ?? null;
  }

  function addMember(name: string) {
    return openGroup.change((groupId) => api.addParticipant(groupId, name));
  }

  async function renameMember(name: string) {
    const member = dialogs.renameMember.member;
    if (member)
      await openGroup.change((groupId) => api.renameParticipant(groupId, member.id, name));
  }
</script>

<ModeWatcher modeStorageKey="theme" />
<ConfirmDialog />
<!-- For an invite once logged in, and before that for a code that logs in. -->
{#if dialogs.scanning}
  <QrScanOverlay onCancel={stopScanning} />
{/if}

{#if session.account === undefined}
  <Splash />
{:else if session.account === null}
  <AuthScreen {onAuthenticated} />
{:else}
  {@const account = session.account}
  <!-- The page scrolls between the top bar and, on phones, the bottom one: its scrollbar is
       there, and opening a menu or a dialog (which stops the window from scrolling) moves
       neither bar. -->
  <div class="flex h-dvh flex-col">
    <Navbar
      inGroup={openGroup.group !== null}
      onNavigateHome={goHome}
      onOpenCreateGroup={() => (dialogs.createGroup = true)}
      username={account.username}
    />

    <div class="min-h-0 flex-1 overflow-y-auto max-sm:mb-(--bottom-bar-room)">
      <main
        class="mx-auto w-full max-w-6xl space-y-6 px-4 py-6 sm:pb-[max(1.5rem,env(safe-area-inset-bottom))]"
      >
        {#if storageWarnings.length > 0}
          <Alert.Root>
            <TriangleAlertIcon />
            <Alert.Title>{t("app.storageWarnings")}</Alert.Title>
            <Alert.Description>
              <ul class="list-disc pl-4">
                {#each storageWarnings as warning (warning)}
                  <li>{backendText(warning)}</li>
                {/each}
              </ul>
              <p>{t("app.storageKept")}</p>
            </Alert.Description>
            <Alert.Action>
              <Button variant="ghost" size="sm" onclick={() => (storageWarnings = [])}>
                {t("common.dismiss")}
              </Button>
            </Alert.Action>
          </Alert.Root>
        {/if}

        {#if groupList.loading}
          <div class="flex items-center justify-center gap-2 py-20 text-sm text-muted-foreground">
            <Spinner />
            {t("app.loadingGroups")}
          </div>
        {:else if !openGroup.group}
          <GroupDashboard
            groups={groupList.all}
            onSelectGroup={(groupId) => navigation.open(groupId)}
            onOpenCreateGroup={() => (dialogs.createGroup = true)}
            onOpenJoinGroup={() => dialogs.openJoin()}
          />
          <BottomBar home onHome={goHome} class="sm:hidden">
            <Button onclick={() => (dialogs.createGroup = true)} class="ml-auto">
              <PlusIcon data-icon="inline-start" />
              {t("app.newGroup")}
            </Button>
          </BottomBar>
        {:else}
          <GroupPage group={openGroup.group} />
        {/if}
      </main>
    </div>

    <CreateGroupModal />
    <JoinGroupModal />
    <AccountDialog />
    <ChangePasswordDialog />
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
  </div>
{/if}
