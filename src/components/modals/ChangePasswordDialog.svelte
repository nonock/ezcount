<script lang="ts">
  import { untrack } from "svelte";
  import { toast } from "svelte-sonner";
  import PasswordStrengthMeter from "@/components/auth/PasswordStrengthMeter.svelte";
  import { ratePassword } from "@/components/auth/password-strength.svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { session } from "@/lib/state/session.svelte";
  import { api } from "@/services/api";
  import { errorMessage } from "@/utils/errors";

  let current = $state("");
  let next = $state("");
  let confirm = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);
  const strength = ratePassword(() => ({
    password: next,
    username: session.account?.username ?? "",
    enabled: dialogs.changePassword,
  }));

  $effect.pre(() => {
    if (!dialogs.changePassword) return;
    untrack(() => {
      current = "";
      next = "";
      confirm = "";
      error = null;
    });
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (next !== confirm) {
      error = t("password.mismatch");
      return;
    }
    submitting = true;
    error = null;
    try {
      await api.changePassword(current, next);
      toast.success(t("password.changed"));
      dialogs.changePassword = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog.Root bind:open={dialogs.changePassword}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("password.title")}</Dialog.Title>
      <Dialog.Description>{t("password.intro")}</Dialog.Description>
    </Dialog.Header>
    <form onsubmit={handleSubmit}>
      <Field.Group>
        <Field.Field>
          <Field.Label for="current-password">{t("password.current")}</Field.Label>
          <Input
            id="current-password"
            type="password"
            required
            bind:value={current}
            autocomplete="current-password"
          />
        </Field.Field>
        <Field.Field>
          <Field.Label for="new-password">{t("auth.newPassword")}</Field.Label>
          <Input
            id="new-password"
            type="password"
            required
            minlength={8}
            bind:value={next}
            autocomplete="new-password"
            aria-describedby={strength.current ? "new-password-strength" : undefined}
          />
          {#if strength.current}
            <PasswordStrengthMeter strength={strength.current} id="new-password-strength" />
          {/if}
        </Field.Field>
        <Field.Field>
          <Field.Label for="confirm-new-password">{t("auth.confirmNewPassword")}</Field.Label>
          <Input
            id="confirm-new-password"
            type="password"
            required
            bind:value={confirm}
            autocomplete="new-password"
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
        <Button type="submit" disabled={submitting || !strength.current?.acceptable}>
          {#if submitting}
            <Spinner data-icon="inline-start" />
          {/if}
          {t("password.submit")}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
