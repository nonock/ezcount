<script lang="ts">
  import QrCodeIcon from "@lucide/svelte/icons/qr-code";
  import ScanQrCodeIcon from "@lucide/svelte/icons/scan-qr-code";
  import { untrack } from "svelte";
  import { toast } from "svelte-sonner";
  import ReceiveCode from "@/components/common/ReceiveCode.svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Spinner } from "@/components/ui/spinner";
  import { Textarea } from "@/components/ui/textarea";
  import { joinGroup } from "@/lib/actions/groups";
  import { scanInvite } from "@/lib/actions/scan";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { groupList } from "@/lib/state/groups.svelte";
  import { navigation } from "@/lib/state/navigation.svelte";
  import { session } from "@/lib/state/session.svelte";
  import { nativeFeatures } from "@/services/native.svelte";
  import type { Received } from "@/types";
  import { errorMessage } from "@/utils/errors";

  let code = $state("");
  // Shows a code for a phone that is in the group to scan, instead of asking for a link.
  let receiving = $state(false);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Opens with the invite from a link or a scan, if any, and why joining with it failed.
  $effect.pre(() => {
    if (!dialogs.join.open) return;
    untrack(() => {
      code = dialogs.join.code;
      error = dialogs.join.error;
      receiving = false;
    });
  });

  /** The phone sent a group's invite, and this device joined it. */
  async function received({ group }: Received) {
    dialogs.join.open = false;
    if (!group) return;
    await groupList.refresh();
    navigation.open(group.id);
    toast.success(t("groups.joined", group.name));
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (!code.trim()) return;
    submitting = true;
    error = null;
    try {
      await joinGroup(code.trim());
      dialogs.join.open = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog.Root bind:open={dialogs.join.open}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("join.title")}</Dialog.Title>
      <Dialog.Description>
        {receiving ? t("receive.joinHelp") : t("join.intro")}
      </Dialog.Description>
    </Dialog.Header>
    {#if receiving && session.account}
      <ReceiveCode serverUrl={session.account.server_url} purpose="group" onReceived={received} />
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (receiving = false)}>{t("common.back")}</Button>
      </Dialog.Footer>
    {:else}
      <form onsubmit={handleSubmit}>
        <Field.Group>
          <Button variant="outline" size="lg" onclick={() => (receiving = true)}>
            <QrCodeIcon data-icon="inline-start" />
            {t("receive.join")}
          </Button>
          {#if nativeFeatures.scan}
            <Button variant="outline" size="lg" onclick={scanInvite}>
              <ScanQrCodeIcon data-icon="inline-start" />
              {t("join.scan")}
            </Button>
          {/if}
          <Field.Field>
            <Field.Label for="input-invite-code">{t("join.link")}</Field.Label>
            <Textarea
              id="input-invite-code"
              required
              rows={3}
              bind:value={code}
              placeholder="https://…/join#…"
              spellcheck={false}
              autocapitalize="off"
              class="resize-none font-mono text-xs break-all"
            />
          </Field.Field>
          {#if error}
            <Field.Error>{error}</Field.Error>
          {/if}
        </Field.Group>
        <Dialog.Footer class="mt-6">
          <Dialog.Close>
            {#snippet child({ props })}
              <Button {...props} variant="outline">{t("common.cancel")}</Button>
            {/snippet}
          </Dialog.Close>
          <Button type="submit" disabled={submitting}>
            {#if submitting}
              <Spinner data-icon="inline-start" />
            {/if}
            {t("join.submit")}
          </Button>
        </Dialog.Footer>
      </form>
    {/if}
  </Dialog.Content>
</Dialog.Root>
