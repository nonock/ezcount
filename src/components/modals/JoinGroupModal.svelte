<script lang="ts">
  import ScanQrCodeIcon from "@lucide/svelte/icons/scan-qr-code";
  import { untrack } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Spinner } from "@/components/ui/spinner";
  import { Textarea } from "@/components/ui/textarea";
  import { joinGroup, scanInvite } from "@/lib/actions";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { nativeFeatures } from "@/services/native.svelte";
  import { errorMessage } from "@/utils/errors";

  let code = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Opens with the invite from a link or a scan, if any, and why joining with it failed.
  $effect.pre(() => {
    if (!dialogs.join.open) return;
    untrack(() => {
      code = dialogs.join.code;
      error = dialogs.join.error;
    });
  });

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
      <Dialog.Description>{t("join.intro")}</Dialog.Description>
    </Dialog.Header>
    <form onsubmit={handleSubmit}>
      <Field.Group>
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
  </Dialog.Content>
</Dialog.Root>
