<script lang="ts">
  import { untrack } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import { deleteAccount } from "@/lib/actions/session";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { session } from "@/lib/state/session.svelte";
  import { errorMessage } from "@/utils/errors";

  /** Deleting the account can't be undone: it says what goes, and takes the password. */

  let password = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  $effect.pre(() => {
    if (!dialogs.deleteAccount) return;
    untrack(() => {
      password = "";
      error = null;
    });
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    submitting = true;
    error = null;
    try {
      await deleteAccount(password);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog.Root bind:open={dialogs.deleteAccount}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("deleteAccount.title", session.account?.username ?? "")}</Dialog.Title>
      <Dialog.Description>{t("deleteAccount.intro")}</Dialog.Description>
    </Dialog.Header>
    <form onsubmit={handleSubmit}>
      <Field.Group>
        <ul class="list-disc space-y-1 pl-5 text-sm text-muted-foreground">
          <li>{t("deleteAccount.removed")}</li>
          <li>{t("deleteAccount.kept")}</li>
        </ul>
        <Field.Field>
          <Field.Label for="delete-account-password">{t("common.password")}</Field.Label>
          <Input
            id="delete-account-password"
            type="password"
            required
            bind:value={password}
            autocomplete="current-password"
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
        <Button type="submit" variant="destructive" disabled={submitting || !password}>
          {#if submitting}
            <Spinner data-icon="inline-start" />
          {/if}
          {t("deleteAccount.submit")}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
