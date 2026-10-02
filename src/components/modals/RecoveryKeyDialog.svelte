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
  import { api } from "@/services/api";
  import { errorMessage } from "@/utils/errors";

  const INTROS: Record<RecoveryKeyReason, string> = {
    signup:
      "If you forget your password, this key is the only way back into your account: nobody can reset it for you. Keep it in a password manager, or write it down.",
    recovered:
      "Your new password is set. The recovery key you used no longer works, so here is a new one to keep.",
    replace: "Your previous recovery key no longer works. Keep this one instead.",
  };

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
      toast.success("Recovery key copied");
    } catch {
      toast.info("Select the key and copy it manually");
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
      <Dialog.Title>{showingKey ? "Save your recovery key" : "New recovery key"}</Dialog.Title>
      <Dialog.Description>
        {showingKey
          ? INTROS[reason]
          : "A new key replaces your current one, which stops working. Enter your password to continue."}
      </Dialog.Description>
    </Dialog.Header>

    {#if showingKey}
      <div class="space-y-3">
        <output
          class="block rounded-xl border bg-muted/50 px-3 py-4 text-center font-mono text-base font-semibold tracking-wide break-all select-all"
          aria-label="Recovery key"
        >
          {key}
        </output>
        <Button variant="outline" class="w-full" onclick={handleCopy}>
          <CopyIcon data-icon="inline-start" />
          Copy Key
        </Button>
        <div class="flex items-center gap-2.5 pt-1">
          <Checkbox id="recovery-key-saved" bind:checked={saved} />
          <Label for="recovery-key-saved" class="font-normal">I've saved it somewhere safe</Label>
        </div>
      </div>
      <Dialog.Footer>
        <Button onclick={onClose} disabled={!saved}>Done</Button>
      </Dialog.Footer>
    {:else}
      <form onsubmit={handleCreate}>
        <Field.Group>
          <Field.Field>
            <Field.Label for="recovery-password">Password</Field.Label>
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
          <Button variant="outline" onclick={onClose}>Cancel</Button>
          <Button type="submit" disabled={submitting}>
            {#if submitting}
              <Spinner data-icon="inline-start" />
            {/if}
            Create New Key
          </Button>
        </Dialog.Footer>
      </form>
    {/if}
  </Dialog.Content>
</Dialog.Root>
