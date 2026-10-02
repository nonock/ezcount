<script lang="ts">
  import CopyIcon from "@lucide/svelte/icons/copy";
  import LockIcon from "@lucide/svelte/icons/lock";
  import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
  import Share2Icon from "@lucide/svelte/icons/share-2";
  import TriangleAlertIcon from "@lucide/svelte/icons/triangle-alert";
  import { toast } from "svelte-sonner";
  import { renderSVG } from "uqr";
  import * as Alert from "@/components/ui/alert";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Spinner } from "@/components/ui/spinner";
  import { Textarea } from "@/components/ui/textarea";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { api } from "@/services/api";
  import { nativeFeatures } from "@/services/native.svelte";
  import type { Group } from "@/types";
  import { errorMessage } from "@/utils/errors";
  import { formatDateTime } from "@/utils/formatters";

  let { group }: { group: Group } = $props();

  let busy = $state(false);
  const syncInfo = $derived(openGroup.syncInfo);
  const invite = $derived(syncInfo?.invite_code ?? null);
  // Dark on light whatever the theme: scanners expect it.
  const qrCode = $derived(
    invite &&
      `data:image/svg+xml;utf8,${encodeURIComponent(renderSVG(invite, { border: 2, blackColor: "#000", whiteColor: "#fff" }))}`
  );
  const canShare = $derived(nativeFeatures.share || typeof navigator.share === "function");

  async function handleSyncNow() {
    busy = true;
    try {
      await openGroup.syncNow();
    } finally {
      busy = false;
    }
  }

  async function handleCopy() {
    if (!invite) return;
    try {
      await navigator.clipboard.writeText(invite);
      toast.success("Invite link copied");
    } catch {
      document.getElementById("share-invite-code")?.focus();
      toast.info("Select the link and copy it manually");
    }
  }

  async function handleShare() {
    if (!invite) return;
    const title = `Join "${group.name}" on ezcount`;
    try {
      if (nativeFeatures.share) {
        await api.shareText(`${title}: ${invite}`, title);
      } else {
        await navigator.share({ title, text: `${title}:`, url: invite });
      }
    } catch (err) {
      // Closing the browser's share sheet without picking an app rejects too.
      if (err instanceof DOMException && err.name === "AbortError") return;
      toast.error("Could not share the invite", { description: errorMessage(err) });
    }
  }
</script>

<Dialog.Root bind:open={dialogs.share}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Invite to Group</Dialog.Title>
      <Dialog.Description>
        Send this link to the people you want in "{group.name}", or let them scan the code from
        ezcount's Join with Code. Everyone can edit, even offline.
      </Dialog.Description>
    </Dialog.Header>

    {#if !invite || !qrCode}
      <div class="flex items-center justify-center gap-2 py-6 text-sm text-muted-foreground">
        <Spinner />
        Loading…
      </div>
    {:else}
      <Field.Group>
        <img
          src={qrCode}
          alt={`QR code of the invite to "${group.name}"`}
          class="mx-auto size-56 rounded-lg"
        />
        <Field.Field>
          <Field.Label for="share-invite-code">Invite link</Field.Label>
          <Textarea
            id="share-invite-code"
            readonly
            rows={3}
            value={invite}
            onfocus={(e) => e.currentTarget.select()}
            class="resize-none font-mono text-xs break-all"
          />
          <Field.Description class="flex items-start gap-1.5">
            <LockIcon class="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
            Anyone with this link can see and edit the group, so share it only with members. Changes are
            end-to-end encrypted: the sync server cannot read them.
          </Field.Description>
        </Field.Field>

        <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
          <dt class="text-muted-foreground">Server</dt>
          <dd class="truncate text-right font-mono">{syncInfo?.server_url}</dd>
          <dt class="text-muted-foreground">Last synced</dt>
          <dd class="text-right">
            {syncInfo?.last_synced_at ? formatDateTime(syncInfo.last_synced_at) : "Never"}
          </dd>
        </dl>

        {#if syncInfo?.last_error}
          <Alert.Root variant="destructive">
            <TriangleAlertIcon />
            <Alert.Description>Last sync failed: {syncInfo.last_error}</Alert.Description>
          </Alert.Root>
        {/if}
      </Field.Group>

      <Dialog.Footer>
        <Button variant="outline" onclick={handleSyncNow} disabled={busy}>
          {#if busy}
            <Spinner data-icon="inline-start" />
          {:else}
            <RefreshCwIcon data-icon="inline-start" />
          {/if}
          Sync Now
        </Button>
        <Button variant={canShare ? "outline" : "default"} onclick={handleCopy}>
          <CopyIcon data-icon="inline-start" />
          Copy Link
        </Button>
        {#if canShare}
          <Button onclick={handleShare}>
            <Share2Icon data-icon="inline-start" />
            Share
          </Button>
        {/if}
      </Dialog.Footer>
    {/if}
  </Dialog.Content>
</Dialog.Root>
