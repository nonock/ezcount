<script lang="ts">
  import UserMinusIcon from "@lucide/svelte/icons/user-minus";
  import { untrack } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import { t } from "@/lib/i18n/index.svelte";
  import type { Participant } from "@/types";
  import { errorMessage } from "@/utils/errors";

  interface Props {
    open: boolean;
    /** The member to rename; adds a new one when absent. */
    member?: Participant | null;
    onSubmit: (name: string) => Promise<void>;
    /** Removes `member` from the group; offered next to renaming. */
    onRemove?: () => void;
  }

  /** Adds a member to the group, or renames one. */
  let { open = $bindable(), member = null, onSubmit, onRemove }: Props = $props();

  let name = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  $effect.pre(() => {
    if (!open) return;
    const initial = member?.name ?? "";
    untrack(() => {
      name = initial;
      error = null;
    });
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    submitting = true;
    error = null;
    try {
      await onSubmit(trimmed);
      open = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-sm">
    <Dialog.Header>
      <Dialog.Title
        >{member ? t("group.renameMember", member.name) : t("member.addTitle")}</Dialog.Title
      >
      <Dialog.Description>
        {member ? t("member.renameIntro") : t("member.addIntro")}
      </Dialog.Description>
    </Dialog.Header>
    <form onsubmit={handleSubmit}>
      <Field.Group>
        <Field.Field>
          <Field.Label for="input-member-name">{t("common.name")}</Field.Label>
          <Input
            id="input-member-name"
            required
            bind:value={name}
            placeholder={t("member.placeholder")}
          />
        </Field.Field>
        {#if error}
          <Field.Error>{error}</Field.Error>
        {/if}
      </Field.Group>
      <Dialog.Footer class="mt-6">
        {#if member && onRemove}
          <Button
            variant="ghost"
            class="text-destructive sm:mr-auto"
            onclick={() => {
              open = false;
              onRemove();
            }}
          >
            <UserMinusIcon data-icon="inline-start" />
            {t("member.remove")}
          </Button>
        {/if}
        <Dialog.Close>
          {#snippet child({ props })}
            <Button {...props} variant="outline">{t("common.cancel")}</Button>
          {/snippet}
        </Dialog.Close>
        <Button type="submit" disabled={submitting}>
          {#if submitting}
            <Spinner data-icon="inline-start" />
          {/if}
          {member ? t("common.rename") : t("member.add")}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
