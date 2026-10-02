<script lang="ts">
  import MinusIcon from "@lucide/svelte/icons/minus";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import { untrack } from "svelte";
  import DatePicker from "@/components/common/DatePicker.svelte";
  import { Button } from "@/components/ui/button";
  import { Checkbox } from "@/components/ui/checkbox";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import * as InputGroup from "@/components/ui/input-group";
  import { Label } from "@/components/ui/label";
  import * as Select from "@/components/ui/select";
  import { Spinner } from "@/components/ui/spinner";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { cn } from "@/lib/utils";
  import { api } from "@/services/api";
  import type { ExpenseSplit, Group } from "@/types";
  import { errorMessage } from "@/utils/errors";
  import { formatDateInput, formatMoney } from "@/utils/formatters";

  /** Adds an expense, or edits `dialogs.expense.editing`. */
  let { group }: { group: Group } = $props();

  interface SplitItemState {
    included: boolean;
    shares: number;
  }

  let title = $state("");
  // A number once typed in: Svelte binds number inputs as numbers.
  let amountStr = $state<string | number | null>("");
  let paidBy = $state("");
  let expenseDate = $state(formatDateInput());
  let splitsState = $state<Record<string, SplitItemState>>({});
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const editing = $derived(dialogs.expense.editing);
  const currentUserId = $derived(openGroup.currentUserId);

  // Removed members stay selectable on expenses they are already part of.
  const participants = $derived.by(() => {
    const involved = new Set(
      editing ? [editing.paid_by, ...editing.splits.map((s) => s.participant_id)] : []
    );
    return group.participants.filter((p) => !p.removed || involved.has(p.id));
  });

  // Initialized once per opening (or when switching to another expense), so a background sync
  // refreshing the group doesn't wipe what the user is typing.
  let initializedFor: string | null = null;
  $effect.pre(() => {
    if (!dialogs.expense.open) {
      initializedFor = null;
      return;
    }
    const key = editing?.id ?? "new";
    untrack(() => {
      if (initializedFor === key) return;
      initializedFor = key;
      error = null;
      const next: Record<string, SplitItemState> = {};
      if (editing) {
        title = editing.title;
        amountStr = (editing.amount_cents / 100).toFixed(2);
        paidBy = editing.paid_by;
        expenseDate = formatDateInput(editing.created_at);
        for (const p of participants) {
          const match = editing.splits.find((s) => s.participant_id === p.id);
          next[p.id] = { included: !!match, shares: match ? match.shares : 1 };
        }
      } else {
        title = "";
        amountStr = "";
        expenseDate = formatDateInput();
        paidBy =
          currentUserId && participants.some((p) => p.id === currentUserId)
            ? currentUserId
            : participants[0]?.id || "";
        for (const p of participants) next[p.id] = { included: true, shares: 1 };
      }
      splitsState = next;
    });
  });

  function toggleParticipant(id: string) {
    const current = splitsState[id];
    splitsState[id] = { included: !current?.included, shares: current?.shares || 1 };
  }

  function updateShares(id: string, shares: number) {
    if (shares < 1) return;
    splitsState[id] = { ...splitsState[id], shares };
  }

  const includedParticipants = $derived(participants.filter((p) => splitsState[p.id]?.included));
  const totalShares = $derived(
    includedParticipants.reduce((sum, p) => sum + (splitsState[p.id]?.shares || 1), 0)
  );
  const allIncluded = $derived(includedParticipants.length === participants.length);

  function toggleAll() {
    const include = !allIncluded;
    const next: Record<string, SplitItemState> = {};
    for (const p of participants) {
      next[p.id] = { included: include, shares: splitsState[p.id]?.shares || 1 };
    }
    splitsState = next;
  }

  const amountCents = $derived.by(() => {
    const decimal = Number.parseFloat(String(amountStr ?? ""));
    return Number.isNaN(decimal) ? 0 : Math.round(decimal * 100);
  });
  const perShareCents = $derived(totalShares > 0 ? Math.floor(amountCents / totalShares) : 0);

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const trimmedTitle = title.trim();
    if (!trimmedTitle) {
      error = "Please enter a description.";
      return;
    }
    if (amountCents <= 0) {
      error = "Please enter an amount greater than zero.";
      return;
    }
    if (includedParticipants.length === 0) {
      error = "Select at least one person to split the bill with.";
      return;
    }

    const splits: ExpenseSplit[] = includedParticipants.map((p) => ({
      participant_id: p.id,
      shares: splitsState[p.id]?.shares || 1,
    }));

    let createdAt: string | null = null;
    if (expenseDate) {
      const [year, month, day] = expenseDate.split("-").map(Number);
      const d = new Date();
      d.setFullYear(year, month - 1, day);
      createdAt = d.toISOString();
    }

    submitting = true;
    error = null;
    const expense = editing;
    try {
      await openGroup.change((groupId) =>
        expense
          ? api.updateExpense(
              groupId,
              expense.id,
              trimmedTitle,
              amountCents,
              paidBy,
              splits,
              createdAt
            )
          : api.addExpense(groupId, trimmedTitle, amountCents, paidBy, splits, createdAt)
      );
      dialogs.expense.open = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }

  const payerName = $derived.by(() => {
    const p = participants.find((x) => x.id === paidBy);
    return p ? `${p.name}${p.id === currentUserId ? " (You)" : ""}` : "Choose…";
  });
</script>

<Dialog.Root bind:open={dialogs.expense.open}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-lg">
    <Dialog.Header>
      <Dialog.Title>{editing ? "Edit Expense" : "Add New Expense"}</Dialog.Title>
      <Dialog.Description>
        {editing
          ? "Changes are recorded in the expense's history."
          : "Who paid, how much, and who it was for."}
      </Dialog.Description>
    </Dialog.Header>

    <form onsubmit={handleSubmit}>
      <Field.Group>
        <Field.Field>
          <Field.Label for="input-expense-title">Description</Field.Label>
          <Input
            id="input-expense-title"
            required
            bind:value={title}
            placeholder="e.g. Groceries, Dinner, Taxi"
          />
        </Field.Field>

        <div class="grid grid-cols-2 gap-4 sm:grid-cols-3">
          <Field.Field>
            <Field.Label for="input-expense-amount">Amount</Field.Label>
            <InputGroup.Root>
              <InputGroup.Input
                id="input-expense-amount"
                type="number"
                inputmode="decimal"
                step="0.01"
                min="0.01"
                required
                bind:value={amountStr}
                placeholder="0.00"
                class="tabular-nums"
              />
              <InputGroup.Addon align="inline-end">
                <InputGroup.Text>{group.currency}</InputGroup.Text>
              </InputGroup.Addon>
            </InputGroup.Root>
          </Field.Field>

          <Field.Field>
            <Field.Label id="label-expense-date" for="input-expense-date">Date</Field.Label>
            <DatePicker
              id="input-expense-date"
              labelId="label-expense-date"
              value={expenseDate}
              onChange={(value) => (expenseDate = value)}
            />
          </Field.Field>

          <Field.Field class="col-span-2 sm:col-span-1">
            <Field.Label for="select-expense-payer">Paid by</Field.Label>
            <Select.Root type="single" bind:value={paidBy}>
              <Select.Trigger id="select-expense-payer" class="w-full">{payerName}</Select.Trigger>
              <Select.Content>
                {#each participants as p (p.id)}
                  <Select.Item
                    value={p.id}
                    label={`${p.name}${p.id === currentUserId ? " (You)" : ""}`}
                  />
                {/each}
              </Select.Content>
            </Select.Root>
          </Field.Field>
        </div>

        <Field.Set>
          <div class="flex items-center justify-between">
            <Field.Legend variant="label" class="mb-0">Split between</Field.Legend>
            <Button variant="link" size="sm" onclick={toggleAll}>
              {allIncluded ? "Deselect all" : "Select all"}
            </Button>
          </div>
          <Field.Description>
            {includedParticipants.length}/{participants.length}
            people, {totalShares}
            {totalShares === 1 ? "part" : "parts"}
            {#if amountCents > 0 && totalShares > 0}
              · about {formatMoney(perShareCents, group.currency)} per part
            {/if}
          </Field.Description>

          <ul class="divide-y rounded-xl border">
            {#each participants as p (p.id)}
              {@const state = splitsState[p.id] || { included: false, shares: 1 }}
              {@const owed =
                totalShares > 0 && amountCents > 0
                  ? Math.round((amountCents * state.shares) / totalShares)
                  : 0}
              <li
                class={cn(
                  "flex min-h-11 items-center justify-between gap-2 px-3 py-1.5",
                  !state.included && "text-muted-foreground"
                )}
              >
                <div class="flex items-center gap-2.5">
                  <Checkbox
                    id={`split-${p.id}`}
                    checked={state.included}
                    onCheckedChange={() => toggleParticipant(p.id)}
                  />
                  <Label for={`split-${p.id}`} class="font-normal">{p.name}</Label>
                </div>

                {#if state.included}
                  <div class="flex items-center gap-2">
                    {#if owed > 0}
                      <span class="text-sm tabular-nums">{formatMoney(owed, group.currency)}</span>
                    {/if}
                    <div class="flex items-center rounded-md border">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        onclick={() => updateShares(p.id, state.shares - 1)}
                        disabled={state.shares <= 1}
                        aria-label={`Fewer parts for ${p.name}`}
                      >
                        <MinusIcon />
                      </Button>
                      <span class="min-w-14 text-center text-xs" aria-live="polite">
                        {state.shares}
                        {state.shares === 1 ? "part" : "parts"}
                      </span>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        onclick={() => updateShares(p.id, state.shares + 1)}
                        aria-label={`More parts for ${p.name}`}
                      >
                        <PlusIcon />
                      </Button>
                    </div>
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        </Field.Set>

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
          {editing ? "Save Changes" : "Save Expense"}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
