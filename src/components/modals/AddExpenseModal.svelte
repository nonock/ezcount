<script lang="ts">
  import ChartPieIcon from "@lucide/svelte/icons/chart-pie";
  import CoinsIcon from "@lucide/svelte/icons/coins";
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
  import { owedAmounts } from "@/lib/split";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { cn } from "@/lib/utils";
  import { api } from "@/services/api";
  import type { ExpenseSplit, Group, OriginalAmount } from "@/types";
  import { CURRENCIES } from "@/utils/currencies";
  import { errorMessage } from "@/utils/errors";
  import { formatDateInput, formatMoney } from "@/utils/formatters";

  /** Adds an expense, or edits `dialogs.expense.editing`. */
  let { group }: { group: Group } = $props();

  // A number once typed in: Svelte binds number inputs as numbers.
  type NumberField = string | number | null;

  interface SplitItemState {
    included: boolean;
    shares: number;
    /** Owes `amount` instead of parts of the rest. */
    fixed: boolean;
    amount: NumberField;
  }

  const toCents = (value: NumberField) => {
    const decimal = Number.parseFloat(String(value ?? ""));
    return Number.isNaN(decimal) ? 0 : Math.round(decimal * 100);
  };

  let title = $state("");
  // In `currency`.
  let amountStr = $state<NumberField>("");
  // svelte-ignore state_referenced_locally
  let currency = $state(group.currency);
  // Units of the group's currency for one of `currency`.
  let rateStr = $state<NumberField>("");
  // Typed by the user, or saved with the expense: a suggestion never replaces it.
  let rateIsOwn = $state(false);
  let lookingUpRate = $state(false);
  // Only the last lookup's answer is used.
  let rateLookup = 0;
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

  // The group's currency first, then the usual ones and the one this expense is already in.
  const currencies = $derived([
    ...new Set([
      group.currency,
      ...CURRENCIES.map((c) => c.code),
      ...(editing?.original ? [editing.original.currency] : []),
    ]),
  ]);

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
      rateLookup += 1;
      lookingUpRate = false;
      const next: Record<string, SplitItemState> = {};
      if (editing) {
        title = editing.title;
        amountStr = ((editing.original?.amount_cents ?? editing.amount_cents) / 100).toFixed(2);
        currency = editing.original?.currency ?? group.currency;
        rateStr = editing.original?.rate ?? "";
        rateIsOwn = !!editing.original;
        paidBy = editing.paid_by;
        expenseDate = formatDateInput(editing.created_at);
        for (const p of participants) {
          const match = editing.splits.find((s) => s.participant_id === p.id);
          const fixed = match?.fixed_cents ?? null;
          next[p.id] = {
            included: !!match,
            shares: match?.shares || 1,
            fixed: fixed !== null,
            amount: fixed !== null ? (fixed / 100).toFixed(2) : "",
          };
        }
      } else {
        title = "";
        amountStr = "";
        currency = group.currency;
        rateStr = "";
        rateIsOwn = false;
        expenseDate = formatDateInput();
        paidBy =
          currentUserId && participants.some((p) => p.id === currentUserId)
            ? currentUserId
            : participants[0]?.id || "";
        for (const p of participants) {
          next[p.id] = { included: true, shares: 1, fixed: false, amount: "" };
        }
      }
      splitsState = next;
    });
  });

  /**
   * Fills in the relay's rate for the expense's day, or else the one last used in the group
   * for that currency. Only suggests: a rate that is the user's own stays.
   */
  async function suggestRate() {
    if (currency === group.currency || rateIsOwn) return;
    const lookup = ++rateLookup;
    const from = currency;
    lookingUpRate = true;
    let suggestion: string | null = null;
    try {
      suggestion = await api.suggestExchangeRate(from, group.currency, expenseDate || null);
    } catch (err) {
      console.warn("No exchange rate from the relay:", err);
    }
    if (lookup !== rateLookup) return;
    lookingUpRate = false;
    if (rateIsOwn) return;
    const last = group.expenses.filter((e) => e.original?.currency === from).at(-1);
    rateStr = suggestion ?? last?.original?.rate ?? "";
  }

  function chooseCurrency(next: string) {
    currency = next;
    rateStr = "";
    rateIsOwn = false;
    rateLookup += 1;
    lookingUpRate = false;
    suggestRate();
  }

  /** The rate follows the day, as long as it is a suggestion. */
  function chooseDate(next: string) {
    expenseDate = next;
    suggestRate();
  }

  /** Also for someone who joined the group while the dialog is open. */
  function stateOf(id: string): SplitItemState {
    splitsState[id] ??= { included: false, shares: 1, fixed: false, amount: "" };
    return splitsState[id];
  }

  function toggleParticipant(id: string) {
    const state = stateOf(id);
    state.included = !state.included;
  }

  function updateShares(id: string, shares: number) {
    if (shares >= 1) stateOf(id).shares = shares;
  }

  const includedParticipants = $derived(participants.filter((p) => splitsState[p.id]?.included));
  const allIncluded = $derived(includedParticipants.length === participants.length);

  function toggleAll() {
    const include = !allIncluded;
    for (const p of participants) stateOf(p.id).included = include;
  }

  const foreign = $derived(currency !== group.currency);
  /** What was paid, in `currency`. */
  const paidCents = $derived(toCents(amountStr));
  const rate = $derived(Number.parseFloat(String(rateStr ?? "").replace(",", ".")));
  /** The same in the group's currency. */
  const amountCents = $derived(foreign ? (rate > 0 ? Math.round(paidCents * rate) : 0) : paidCents);

  const splits = $derived<ExpenseSplit[]>(
    includedParticipants.map((p) => {
      const state = splitsState[p.id];
      return state.fixed
        ? { participant_id: p.id, shares: 0, fixed_cents: toCents(state.amount) }
        : { participant_id: p.id, shares: state.shares, fixed_cents: null };
    })
  );
  const totalShares = $derived(splits.reduce((sum, s) => sum + s.shares, 0));
  const fixedCents = $derived(splits.reduce((sum, s) => sum + (s.fixed_cents ?? 0), 0));
  const anyFixed = $derived(splits.some((s) => s.fixed_cents != null));
  /** What the parts share, in `currency`. Negative when the fixed amounts are too much. */
  const restCents = $derived(paidCents - fixedCents);
  /** What each included person owes, in `currency`. */
  const owed = $derived.by(() => {
    const amounts = owedAmounts(paidCents, null, splits);
    return new Map(splits.map((s, i) => [s.participant_id, amounts[i]]));
  });

  // Why the split doesn't work out, as soon as it shows.
  const splitProblem = $derived.by(() => {
    if (!anyFixed || paidCents <= 0) return null;
    if (restCents < 0) {
      return `The amounts are ${formatMoney(-restCents, currency)} more than the expense.`;
    }
    if (totalShares === 0 && restCents > 0) {
      return `${formatMoney(restCents, currency)} is left to assign.`;
    }
    return null;
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const trimmedTitle = title.trim();
    if (!trimmedTitle) {
      error = "Please enter a description.";
      return;
    }
    if (paidCents <= 0) {
      error = "Please enter an amount greater than zero.";
      return;
    }
    if (foreign && !(rate > 0)) {
      error = `Please enter the exchange rate from ${currency} to ${group.currency}.`;
      return;
    }
    if (amountCents <= 0) {
      error = `This is less than a cent in ${group.currency}.`;
      return;
    }
    if (includedParticipants.length === 0) {
      error = "Select at least one person to split the bill with.";
      return;
    }
    if (splits.some((s) => s.fixed_cents != null && s.fixed_cents <= 0)) {
      error = "Enter an amount for everyone who owes a set amount, or give them parts.";
      return;
    }
    if (splitProblem) {
      error = splitProblem;
      return;
    }

    const original: OriginalAmount | null = foreign
      ? { currency, amount_cents: paidCents, rate: String(rateStr).trim() }
      : null;

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
              createdAt,
              original
            )
          : api.addExpense(groupId, trimmedTitle, amountCents, paidBy, splits, createdAt, original)
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

        <div class="grid grid-cols-2 gap-4">
          <Field.Field>
            <Field.Label for="input-expense-amount">Amount</Field.Label>
            <Input
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
          </Field.Field>

          <Field.Field>
            <Field.Label for="select-expense-currency">Currency</Field.Label>
            <Select.Root type="single" value={currency} onValueChange={chooseCurrency}>
              <Select.Trigger id="select-expense-currency" class="w-full">{currency}</Select.Trigger
              >
              <Select.Content>
                {#each currencies as code (code)}
                  <Select.Item value={code} label={code} />
                {/each}
              </Select.Content>
            </Select.Root>
          </Field.Field>

          {#if foreign}
            <Field.Field class="col-span-2">
              <Field.Label for="input-expense-rate">Exchange rate</Field.Label>
              <InputGroup.Root>
                <InputGroup.Addon>
                  <InputGroup.Text>1 {currency} =</InputGroup.Text>
                </InputGroup.Addon>
                <InputGroup.Input
                  id="input-expense-rate"
                  type="number"
                  inputmode="decimal"
                  step="any"
                  min="0"
                  required
                  bind:value={rateStr}
                  oninput={() => (rateIsOwn = true)}
                  placeholder={lookingUpRate ? "Looking up…" : "0.92"}
                  class="tabular-nums"
                />
                <InputGroup.Addon align="inline-end">
                  <InputGroup.Text>{group.currency}</InputGroup.Text>
                </InputGroup.Addon>
              </InputGroup.Root>
              <Field.Description>
                {#if amountCents > 0}
                  Counts as {formatMoney(amountCents, group.currency)} in the group.
                {:else}
                  The group counts in {group.currency}.
                {/if}
                {#if !rateIsOwn && rate > 0}
                  Suggested rate: change it if you got another.
                {/if}
              </Field.Description>
            </Field.Field>
          {/if}

          <Field.Field>
            <Field.Label id="label-expense-date" for="input-expense-date">Date</Field.Label>
            <DatePicker
              id="input-expense-date"
              labelId="label-expense-date"
              value={expenseDate}
              onChange={chooseDate}
            />
          </Field.Field>

          <Field.Field>
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
            people{#if totalShares > 0}, {totalShares}
              {totalShares === 1 ? "part" : "parts"}
              {#if restCents > 0}
                · about {formatMoney(Math.floor(restCents / totalShares), currency)} per part
              {/if}
            {/if}
            {#if splitProblem}
              <span class="text-destructive">· {splitProblem}</span>
            {/if}
          </Field.Description>

          <ul class="divide-y rounded-xl border">
            {#each participants as p (p.id)}
              {@const state = splitsState[p.id] || {
                included: false,
                shares: 1,
                fixed: false,
                amount: "",
              }}
              {@const owes = owed.get(p.id) ?? 0}
              <li
                class={cn(
                  "flex min-h-11 items-center justify-between gap-2 px-3 py-1.5",
                  !state.included && "text-muted-foreground"
                )}
              >
                <div class="flex min-w-0 items-center gap-2.5">
                  <Checkbox
                    id={`split-${p.id}`}
                    checked={state.included}
                    onCheckedChange={() => toggleParticipant(p.id)}
                  />
                  <Label for={`split-${p.id}`} class="truncate font-normal">{p.name}</Label>
                </div>

                {#if state.included}
                  <div class="flex shrink-0 items-center gap-1.5">
                    {#if state.fixed}
                      <Input
                        type="number"
                        inputmode="decimal"
                        step="0.01"
                        min="0.01"
                        bind:value={splitsState[p.id].amount}
                        placeholder="0.00"
                        aria-label={`Amount for ${p.name}`}
                        class="h-8 w-24 text-right tabular-nums"
                      />
                    {:else}
                      {#if owes > 0}
                        <span class="text-sm tabular-nums">{formatMoney(owes, currency)}</span>
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
                    {/if}
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      onclick={() => (splitsState[p.id].fixed = !state.fixed)}
                      aria-label={state.fixed
                        ? `Give ${p.name} parts of the rest`
                        : `Set an amount for ${p.name}`}
                      title={state.fixed ? "Share the rest by parts" : "Set an amount"}
                    >
                      {#if state.fixed}
                        <ChartPieIcon />
                      {:else}
                        <CoinsIcon />
                      {/if}
                    </Button>
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
