<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { toast } from "svelte-sonner";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import { Textarea } from "@/components/ui/textarea";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { api } from "@/services/api";
  import { errorMessage } from "@/utils/errors";

  /** An idea or a problem, sent to whoever runs the account's server. */
  let message = $state("");
  let contact = $state("");
  let sending = $state(false);
  let error = $state<string | null>(null);

  /** Which app the message comes from: its version, and where it runs. */
  async function appName() {
    // The web version has none of its own: it is the server's.
    const version = await getVersion().catch(() => null);
    return `ezcount ${version ?? "web"} · ${navigator.userAgent}`.slice(0, 200);
  }

  async function send(e: SubmitEvent) {
    e.preventDefault();
    if (!message.trim()) return;
    sending = true;
    error = null;
    try {
      await api.sendFeedback(message.trim(), contact.trim() || null, await appName());
      message = "";
      contact = "";
      dialogs.feedback = false;
      toast.success(t("feedback.sent"));
    } catch (err) {
      error = errorMessage(err);
    } finally {
      sending = false;
    }
  }
</script>

<Dialog.Root bind:open={dialogs.feedback}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("menu.suggest")}</Dialog.Title>
      <Dialog.Description>{t("feedback.intro")}</Dialog.Description>
    </Dialog.Header>
    <form onsubmit={send}>
      <Field.Group>
        <Field.Field>
          <Field.Label for="input-feedback-message">{t("feedback.message")}</Field.Label>
          <Textarea
            id="input-feedback-message"
            bind:value={message}
            required
            maxlength={2000}
            rows={5}
            placeholder={t("feedback.placeholder")}
          />
        </Field.Field>
        <Field.Field>
          <Field.Label for="input-feedback-contact">{t("feedback.contact")}</Field.Label>
          <Input
            id="input-feedback-contact"
            bind:value={contact}
            maxlength={200}
            autocomplete="email"
          />
          <Field.Description>{t("feedback.contactHelp")}</Field.Description>
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
        <Button type="submit" disabled={sending || !message.trim()}>
          {#if sending}
            <Spinner data-icon="inline-start" />
          {/if}
          {t("feedback.send")}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
