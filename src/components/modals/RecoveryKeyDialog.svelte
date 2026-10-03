<script lang="ts" module>
  /** Why a recovery key is on screen. */
  export type RecoveryKeyReason = "signup" | "recovered" | "replace";
</script>

<script lang="ts">
  import CopyIcon from "@lucide/svelte/icons/copy";
  import { untrack } from "svelte";
  import { toast } from "svelte-sonner";
  import { Button } from "@/components/ui/button";
  import { Checkbox } from "@/components/ui/checkbox";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Label } from "@/components/ui/label";
  import { Spinner } from "@/components/ui/spinner";
  import { t } from "@/lib/i18n/index.svelte";
  import { api } from "@/services/api";
  import { errorMessage } from "@/utils/errors";

  const INTROS = {
    signup: "recovery.signup",
    recovered: "recovery.recovered",
    replace: "recovery.replace",
  } as const;

  interface Props {
    open: boolean;
    onClose: () => void;
    reason: RecoveryKeyReason;
    /** The key to show. Without it (reason "replace"), the dialog asks for the password first. */
    recoveryKey?: string | null;
  }

  /**
   * Shows a recovery key once, and stays up until the user confirms they saved it. For
   * "replace", it first makes a new key, which needs the password.
   */
  let { open, onClose, reason, recoveryKey = null }: Props = $props();

  let key = $state<string | null>(null);
  let password = $state("");
  let saved = $state(false);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  $effect.pre(() => {
    if (!open) return;
    const initial = recoveryKey;
    untrack(() => {
      key = initial;
      password = "";
      saved = false;
      error = null;
    });
  });

  // Once a key is on screen, only "Done" closes the dialog: it won't be shown again.
  const showingKey = $derived(key !== null);

  async function handleCreate(e: SubmitEvent) {
    e.preventDefault();
    submitting = true;
    error = null;
    try {
      key = await api.replaceRecoveryKey(password);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }

  async function handleCopy() {
    if (!key) return;
    try {
      await navigator.clipboard.writeText(key);
      toast.success(t("recovery.copied"));
    } catch {
      toast.info(t("recovery.copyManually"));
    }
  }
</script>

<Dialog.Root
  bind:open={
    () => open,
    (next) => {
      if (!next && !showingKey) onClose();
    }
  }
>
  <Dialog.Content
    class="sm:max-w-md"
    showCloseButton={!showingKey}
    escapeKeydownBehavior={showingKey ? "ignore" : "close"}
    interactOutsideBehavior={showingKey ? "ignore" : "close"}
  >
    <Dialog.Header>
      <Dialog.Title>{showingKey ? t("recovery.saveTitle") : t("menu.newRecoveryKey")}</Dialog.Title>
      <Dialog.Description>
        {showingKey ? t(INTROS[reason]) : t("recovery.newIntro")}
      </Dialog.Description>
    </Dialog.Header>

    {#if showingKey}
      <div class="space-y-3">
        <output
          class="block rounded-xl border bg-muted/50 px-3 py-4 text-center font-mono text-base font-semibold tracking-wide break-all select-all"
          aria-label={t("auth.recoveryKey")}
        >
          {key}
        </output>
        <Button variant="outline" class="w-full" onclick={handleCopy}>
          <CopyIcon data-icon="inline-start" />
          {t("recovery.copy")}
        </Button>
        <div class="flex items-center gap-2.5 pt-1">
          <Checkbox id="recovery-key-saved" bind:checked={saved} />
          <Label for="recovery-key-saved" class="font-normal">{t("recovery.saved")}</Label>
        </div>
      </div>
      <Dialog.Footer>
        <Button onclick={onClose} disabled={!saved}>{t("common.done")}</Button>
      </Dialog.Footer>
    {:else}
      <form onsubmit={handleCreate}>
        <Field.Group>
          <Field.Field>
            <Field.Label for="recovery-password">{t("common.password")}</Field.Label>
            <Input
              id="recovery-password"
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
          <Button variant="outline" onclick={onClose}>{t("common.cancel")}</Button>
          <Button type="submit" disabled={submitting}>
            {#if submitting}
              <Spinner data-icon="inline-start" />
            {/if}
            {t("recovery.create")}
          </Button>
        </Dialog.Footer>
      </form>
    {/if}
  </Dialog.Content>
</Dialog.Root>
