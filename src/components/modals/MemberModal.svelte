<script lang="ts">
  import { untrack } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import type { Participant } from "@/types";
  import { errorMessage } from "@/utils/errors";

  interface Props {
    open: boolean;
    /** The member to rename; adds a new one when absent. */
    member?: Participant | null;
    onSubmit: (name: string) => Promise<void>;
  }

  /** Adds a member to the group, or renames one. */
  let { open = $bindable(), member = null, onSubmit }: Props = $props();

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
      <Dialog.Title>{member ? `Rename ${member.name}` : "Add Group Member"}</Dialog.Title>
      <Dialog.Description>
        {member
          ? "Their expenses and payments show the new name too."
          : "They can be included in expenses right away."}
      </Dialog.Description>
    </Dialog.Header>
    <form onsubmit={handleSubmit}>
      <Field.Group>
        <Field.Field>
          <Field.Label for="input-member-name">Name</Field.Label>
          <Input id="input-member-name" required bind:value={name} placeholder="e.g. David" />
        </Field.Field>
        {#if error}
          <Field.Error>{error}</Field.Error>
        {/if}
      </Field.Group>
      <Dialog.Footer class="mt-6">
        <Dialog.Close>
          {#snippet child({ props })}
            <Button {...props} variant="outline">Cancel</Button>
          {/snippet}
        </Dialog.Close>
        <Button type="submit" disabled={submitting}>
          {#if submitting}
            <Spinner data-icon="inline-start" />
          {/if}
          {member ? "Rename" : "Add to Group"}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
