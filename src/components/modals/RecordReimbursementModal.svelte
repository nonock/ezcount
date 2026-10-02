<script lang="ts">
  import { untrack } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import * as InputGroup from "@/components/ui/input-group";
  import * as Select from "@/components/ui/select";
  import { Spinner } from "@/components/ui/spinner";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { api } from "@/services/api";
  import type { Group } from "@/types";
  import { errorMessage } from "@/utils/errors";

  let { group }: { group: Group } = $props();

  let fromId = $state("");
  let toId = $state("");
  // A number once typed in: Svelte binds number inputs as numbers.
  let amountStr = $state<string | number | null>("");
  let notes = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Initialized once per opening, so a background sync refreshing the group keeps the form.
  $effect.pre(() => {
    if (!dialogs.reimburse.open) return;
    untrack(() => {
      const prefill = dialogs.reimburse;
      const active = group.participants.filter((p) => !p.removed);
      const pool = active.length > 0 ? active : group.participants;
      const p1 = pool[0]?.id ?? "";
      const p2 = pool.length > 1 ? pool[1].id : p1;
      fromId = prefill.fromId || p1;
      toId = prefill.toId || (prefill.fromId === p1 ? p2 : p1);
      amountStr = prefill.amount;
      notes = "";
      error = null;
    });
  });

  const nameOf = (id: string) => {
    const p = group.participants.find((x) => x.id === id);
    return p ? `${p.name}${p.removed ? " (removed)" : ""}` : "";
  };

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (fromId === toId) {
      error = "The sender and recipient cannot be the same person.";
      return;
    }
    const amountDecimal = Number.parseFloat(String(amountStr ?? ""));
    const amountCents = !Number.isNaN(amountDecimal) ? Math.round(amountDecimal * 100) : 0;
    if (amountCents <= 0) {
      error = "Please enter an amount greater than zero.";
      return;
    }
    submitting = true;
    error = null;
    try {
      await openGroup.change((groupId) =>
        api.recordReimbursement(groupId, fromId, toId, amountCents, notes.trim() || undefined)
      );
      dialogs.reimburse.open = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

{#snippet options()}
  <Select.Content>
    {#each group.participants as p (p.id)}
      <Select.Item value={p.id} label={nameOf(p.id)} />
    {/each}
  </Select.Content>
{/snippet}

<Dialog.Root bind:open={dialogs.reimburse.open}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Record Reimbursement</Dialog.Title>
      <Dialog.Description>Record money paid back between two members.</Dialog.Description>
    </Dialog.Header>

    <form onsubmit={handleSubmit}>
      <Field.Group>
        <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <Field.Field>
            <Field.Label for="select-reimburse-from">From (sender)</Field.Label>
            <Select.Root type="single" bind:value={fromId}>
              <Select.Trigger id="select-reimburse-from" class="w-full">
                {nameOf(fromId)}
              </Select.Trigger>
              {@render options()}
            </Select.Root>
          </Field.Field>
          <Field.Field>
            <Field.Label for="select-reimburse-to">To (recipient)</Field.Label>
            <Select.Root type="single" bind:value={toId}>
              <Select.Trigger id="select-reimburse-to" class="w-full">
                {nameOf(toId)}
              </Select.Trigger>
              {@render options()}
            </Select.Root>
          </Field.Field>
        </div>

        <Field.Field>
          <Field.Label for="input-reimburse-amount">Amount</Field.Label>
          <InputGroup.Root>
            <InputGroup.Input
              id="input-reimburse-amount"
              type="number"
              inputmode="decimal"
              step="0.01"
              min="0.01"
              placeholder="0.00"
              required
              bind:value={amountStr}
              class="tabular-nums"
            />
            <InputGroup.Addon align="inline-end">
              <InputGroup.Text>{group.currency}</InputGroup.Text>
            </InputGroup.Addon>
          </InputGroup.Root>
        </Field.Field>

        <Field.Field>
          <Field.Label for="input-reimburse-notes">Note (optional)</Field.Label>
          <Input
            id="input-reimburse-notes"
            placeholder="e.g. Bank transfer, Cash"
            bind:value={notes}
          />
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
          Confirm Payment
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
