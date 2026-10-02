<script lang="ts">
  import { untrack } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import * as Select from "@/components/ui/select";
  import { Spinner } from "@/components/ui/spinner";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { api } from "@/services/api";
  import type { Group } from "@/types";
  import { CURRENCIES } from "@/utils/currencies";
  import { errorMessage } from "@/utils/errors";

  let { group }: { group: Group } = $props();

  let name = $state("");
  let currency = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  $effect.pre(() => {
    if (!dialogs.editGroup) return;
    const initial = { name: group.name, currency: group.currency };
    untrack(() => {
      name = initial.name;
      currency = initial.currency;
      error = null;
    });
  });

  // A group made elsewhere may use a currency this list doesn't offer.
  const options = $derived(
    CURRENCIES.some((c) => c.code === group.currency)
      ? CURRENCIES
      : [{ code: group.currency, label: group.currency }, ...CURRENCIES]
  );

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) {
      error = "Please enter a group name.";
      return;
    }
    submitting = true;
    error = null;
    try {
      await openGroup.change((groupId) => api.updateGroup(groupId, trimmed, currency));
      dialogs.editGroup = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog.Root bind:open={dialogs.editGroup}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Edit Group</Dialog.Title>
      <Dialog.Description>Changes show up for every member of the group.</Dialog.Description>
    </Dialog.Header>
    <form onsubmit={handleSubmit}>
      <Field.Group>
        <Field.Field>
          <Field.Label for="input-edit-group-name">Group name</Field.Label>
          <Input id="input-edit-group-name" required bind:value={name} />
        </Field.Field>
        <Field.Field>
          <Field.Label for="select-edit-group-currency">Currency</Field.Label>
          <Select.Root type="single" bind:value={currency}>
            <Select.Trigger id="select-edit-group-currency" class="w-full">
              {options.find((c) => c.code === currency)?.label ?? currency}
            </Select.Trigger>
            <Select.Content>
              {#each options as c (c.code)}
                <Select.Item value={c.code} label={c.label} />
              {/each}
            </Select.Content>
          </Select.Root>
          <Field.Description>
            Amounts stay as they are: 10 € becomes 10 in the new currency, not converted.
          </Field.Description>
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
          Save
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
