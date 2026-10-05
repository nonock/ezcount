<script lang="ts">
  import PlusIcon from "@lucide/svelte/icons/plus";
  import { ModeWatcher, mode } from "mode-watcher";
  import { untrack } from "svelte";
  import { toast } from "svelte-sonner";
  import AuthScreen from "@/components/auth/AuthScreen.svelte";
  import BottomBar from "@/components/common/BottomBar.svelte";
  import ConfirmDialog from "@/components/common/ConfirmDialog.svelte";
  import Navbar from "@/components/common/Navbar.svelte";
  import PageScroller from "@/components/common/PageScroller.svelte";
  import QrScanOverlay from "@/components/common/QrScanOverlay.svelte";
  import Splash from "@/components/common/Splash.svelte";
  import StorageWarnings from "@/components/common/StorageWarnings.svelte";
  import UpdateNotice from "@/components/common/UpdateNotice.svelte";
  import GroupDashboard from "@/components/dashboard/GroupDashboard.svelte";
  import AppDialogs from "@/components/modals/AppDialogs.svelte";
  import { Button } from "@/components/ui/button";
  import { Spinner } from "@/components/ui/spinner";
  import GroupPage from "@/components/workspace/GroupPage.svelte";
  import { goHome, joinGroup } from "@/lib/actions/groups";
  import { followBackend } from "@/lib/backendEvents.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { hideAppBehindModals } from "@/lib/modalBackdrop.svelte";
  import { savedInvite, saveInvite } from "@/lib/pendingInvite";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { groupList, openGroup } from "@/lib/state/groups.svelte";
  import { navigation } from "@/lib/state/navigation.svelte";
  import { type NewRecoveryKey, session } from "@/lib/state/session.svelte";
  import { paintSystemBars } from "@/lib/systemBars";
  import { api } from "@/services/api";
  import {
    cancelScan,
    handleAndroidBack,
    loadNativeFeatures,
    onInviteLink,
  } from "@/services/native.svelte";
  import type { AccountInfo } from "@/types";
  import { errorMessage } from "@/utils/errors";

  let storageWarnings = $state<string[]>([]);
  // An invite link the app was opened with, kept until the user is logged in.
  let pendingInvite = $state<string | null>(savedInvite());
  // It had to wait for a login: opening it was already the decision to join.
  let waitedForLogin = savedInvite() !== null;

  session.refresh();
  loadNativeFeatures();
  api
    .getStorageWarnings()
    .then((warnings) => (storageWarnings = warnings))
    .catch((err) => console.error("Failed to load storage warnings:", err));

  $effect(() => onInviteLink((link) => (pendingInvite = link)));
  $effect(() => navigation.listen());

  // The phone's own bars take the color of the app's, in the theme shown. A frame later, once
  // the theme's class is on the page.
  $effect(() => {
    void mode.current;
    const surface = session.loggedIn ? "card" : "background";
    const frame = requestAnimationFrame(() => paintSystemBars(surface));
    return () => cancelAnimationFrame(frame);
  });

  $effect(() => {
    // Not while the app still finds out whether someone is logged in.
    if (!pendingInvite || session.account === undefined) return;
    const code = pendingInvite;
    if (session.account === null) {
      waitedForLogin = true;
      saveInvite(code);
      return;
    }
    untrack(() => {
      pendingInvite = null;
      saveInvite(null);
      if (!waitedForLogin) {
        dialogs.openJoin(code);
        return;
      }
      // Joined right after logging in or signing up; what goes wrong shows with the invite.
      waitedForLogin = false;
      joinGroup(code).catch((err) => dialogs.openJoin(code, errorMessage(err)));
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

  followBackend();

  function refreshFailed(err: unknown) {
    toast.error(t("app.refreshFailed"), { description: errorMessage(err) });
  }

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
      !openGroup.group.needs_update &&
      session.account &&
      !openGroup.currentUserId &&
      !dialogs.identitySkipped.has(openGroup.group.id)
    )
  );
  $effect(() => {
    if (needsIdentity) untrack(() => (dialogs.who = true));
  });

  hideAppBehindModals();

  handleAndroidBack(() => (dialogs.scanning ? stopScanning : openGroup.group ? goHome : null));

  function stopScanning() {
    cancelScan().catch((err) => console.error("Could not stop scanning:", err));
  }

  function onAuthenticated(account: AccountInfo, recoveryKey?: NewRecoveryKey) {
    groupList.loading = true;
    session.account = account;
    session.newRecoveryKey = recoveryKey ?? null;
  }
</script>

<!-- The colors are the page's background in each theme (`--background` in styles.css). -->
<ModeWatcher modeStorageKey="theme" themeColors={{ light: "#f8f8f8", dark: "#191e24" }} />
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

    <!-- Pulling a group down from its top brings it up to date with the other devices. -->
    <PageScroller
      class="max-sm:mb-(--bottom-bar-room)"
      onRefresh={openGroup.group ? () => openGroup.syncNow().catch(refreshFailed) : undefined}
    >
      <main
        class="mx-auto w-full max-w-6xl space-y-6 px-4 py-6 sm:pb-[max(1.5rem,env(safe-area-inset-bottom))]"
      >
        <StorageWarnings bind:warnings={storageWarnings} />
        {#if account.update_required}
          <UpdateNotice text={t("update.account")} />
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
    </PageScroller>

    <AppDialogs />
  </div>
{/if}
