<script lang="ts">
  import { type Snippet, untrack } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import * as InputGroup from "@/components/ui/input-group";
  import * as Select from "@/components/ui/select";
  import { Spinner } from "@/components/ui/spinner";
  import { t } from "@/lib/i18n/index.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { api } from "@/services/api";
  import type { Group } from "@/types";
  import { errorMessage } from "@/utils/errors";
  import { currencySymbol } from "@/utils/formatters";

  interface Props {
    group: Group;
    prefill?: { fromId?: string; toId?: string; amount?: string };
    onDone: () => void;
    /** Shown above the fields. */
    children?: Snippet;
  }

  /**
   * Records money one member gave another, inside a dialog: its own, or the expense form's
   * as a transfer. Filled in once, when it appears, so a background sync refreshing the group
   * keeps what is typed.
   */
  let { group, prefill = {}, onDone, children }: Props = $props();

  const start = untrack(() => {
    const active = group.participants.filter((p) => !p.removed);
    const pool = active.length > 0 ? active : group.participants;
    const p1 = pool[0]?.id ?? "";
    const p2 = pool.length > 1 ? pool[1].id : p1;
    const fromId = prefill.fromId || p1;
    return { fromId, toId: prefill.toId || (fromId === p1 ? p2 : p1), amount: prefill.amount };
  });

  let fromId = $state(start.fromId);
  let toId = $state(start.toId);
  // A number once typed in: Svelte binds number inputs as numbers.
  let amountStr = $state<string | number | null>(start.amount ?? "");
  let notes = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const nameOf = (id: string) => {
    const p = group.participants.find((x) => x.id === id);
    return p ? (p.removed ? t("member.withRemoved", p.name) : p.name) : "";
  };

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (fromId === toId) {
      error = t("reimburse.samePerson");
      return;
    }
    const amountDecimal = Number.parseFloat(String(amountStr ?? ""));
    const amountCents = !Number.isNaN(amountDecimal) ? Math.round(amountDecimal * 100) : 0;
    if (amountCents <= 0) {
      error = t("expense.needAmount");
      return;
    }
    submitting = true;
    error = null;
    try {
      await openGroup.change((groupId) =>
        api.recordReimbursement(groupId, fromId, toId, amountCents, notes.trim() || undefined)
      );
      onDone();
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

<form onsubmit={handleSubmit}>
  <Field.Group>
    {@render children?.()}
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
      <Field.Field>
        <Field.Label for="select-reimburse-from">{t("reimburse.from")}</Field.Label>
        <Select.Root type="single" bind:value={fromId}>
          <Select.Trigger id="select-reimburse-from" class="w-full">
            {nameOf(fromId)}
          </Select.Trigger>
          {@render options()}
        </Select.Root>
      </Field.Field>
      <Field.Field>
        <Field.Label for="select-reimburse-to">{t("reimburse.to")}</Field.Label>
        <Select.Root type="single" bind:value={toId}>
          <Select.Trigger id="select-reimburse-to" class="w-full">
            {nameOf(toId)}
          </Select.Trigger>
          {@render options()}
        </Select.Root>
      </Field.Field>
    </div>

    <Field.Field>
      <Field.Label for="input-reimburse-amount">{t("common.amount")}</Field.Label>
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
          <InputGroup.Text>{currencySymbol(group.currency)}</InputGroup.Text>
        </InputGroup.Addon>
      </InputGroup.Root>
    </Field.Field>

    <Field.Field>
      <Field.Label for="input-reimburse-notes">{t("reimburse.note")}</Field.Label>
      <Input
        id="input-reimburse-notes"
        placeholder={t("reimburse.notePlaceholder")}
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
        <Button {...props} variant="outline">{t("common.cancel")}</Button>
      {/snippet}
    </Dialog.Close>
    <Button type="submit" disabled={submitting}>
      {#if submitting}
        <Spinner data-icon="inline-start" />
      {/if}
      {t("reimburse.submit")}
    </Button>
  </Dialog.Footer>
</form>
