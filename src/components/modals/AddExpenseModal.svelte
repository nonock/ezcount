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
  import { t } from "@/lib/i18n/index.svelte";
  import { owedAmounts } from "@/lib/split";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { cn } from "@/lib/utils";
  import { api } from "@/services/api";
  import type { ExpensePayer, ExpenseSplit, Group, OriginalAmount } from "@/types";
  import { CURRENCIES } from "@/utils/currencies";
  import { errorMessage } from "@/utils/errors";
  import { amountInput, formatDateInput, formatMoney } from "@/utils/formatters";

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

  /** One of the people who may have paid part of the expense. */
  interface PayerState {
    included: boolean;
    amount: NumberField;
  }

  /** In the "Paid by" list, after the people. */
  const SEVERAL = "several";

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
  // Several people paid: who, and how much each, in `currency`.
  let severalPayers = $state(false);
  let payersState = $state<Record<string, PayerState>>({});
  let expenseDate = $state(formatDateInput());
  let splitsState = $state<Record<string, SplitItemState>>({});
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const editing = $derived(dialogs.expense.editing);
  const currentUserId = $derived(openGroup.currentUserId);

  // Removed members stay selectable on expenses they are already part of.
  const participants = $derived.by(() => {
    const involved = new Set(
      editing
        ? [
            editing.paid_by,
            ...(editing.payers ?? []).map((p) => p.participant_id),
            ...editing.splits.map((s) => s.participant_id),
          ]
        : []
    );
    return group.participants.filter((p) => !p.removed || involved.has(p.id));
  });

  // The group's currency first, then the usual ones and the one this expense is already in.
  const currencies = $derived([
    ...new Set([
      group.currency,
      ...CURRENCIES,
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
      const nextPayers: Record<string, PayerState> = {};
      const paidTogether = editing?.payers ?? [];
      severalPayers = paidTogether.length > 1;
      for (const p of participants) {
        const paid = paidTogether.find((x) => x.participant_id === p.id);
        nextPayers[p.id] = {
          included: !!paid,
          amount: paid ? amountInput(paid.amount_cents) : "",
        };
      }
      payersState = nextPayers;
      if (editing) {
        title = editing.title;
        amountStr = amountInput(editing.original?.amount_cents ?? editing.amount_cents);
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
            amount: fixed !== null ? amountInput(fixed) : "",
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

  function payerOf(id: string): PayerState {
    payersState[id] ??= { included: false, amount: "" };
    return payersState[id];
  }

  function choosePayer(value: string) {
    if (value !== SEVERAL) {
      severalPayers = false;
      paidBy = value;
      return;
    }
    severalPayers = true;
    // Starts from who was paying alone, with the whole amount.
    if (!participants.some((p) => payersState[p.id]?.included) && paidBy) {
      const state = payerOf(paidBy);
      state.included = true;
      state.amount = paidCents > 0 ? amountInput(paidCents) : "";
    }
  }

  function togglePayer(id: string) {
    const state = payerOf(id);
    state.included = !state.included;
    state.amount = "";
    totalPayers();
  }

  /** With several payers, the expense's amount is what they paid between them. */
  function totalPayers() {
    const total = participants
      .filter((p) => payersState[p.id]?.included)
      .reduce((sum, p) => sum + toCents(payersState[p.id].amount), 0);
    amountStr = total > 0 ? amountInput(total) : "";
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

  const payers = $derived<ExpensePayer[]>(
    participants
      .filter((p) => payersState[p.id]?.included)
      .map((p) => ({ participant_id: p.id, amount_cents: toCents(payersState[p.id].amount) }))
  );

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
      return t("expense.tooMuch", formatMoney(-restCents, currency));
    }
    if (totalShares === 0 && restCents > 0) {
      return t("expense.leftToAssign", formatMoney(restCents, currency));
    }
    return null;
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const trimmedTitle = title.trim();
    if (!trimmedTitle) {
      error = t("expense.needDescription");
      return;
    }
    if (severalPayers && payers.length === 0) {
      error = t("expense.needPayers");
      return;
    }
    // One person ticked among "several" paid it all.
    const together = severalPayers && payers.length > 1 ? payers : [];
    if (together.some((p) => p.amount_cents <= 0)) {
      error = t("expense.needPayerAmounts");
      return;
    }
    if (paidCents <= 0) {
      error = t("expense.needAmount");
      return;
    }
    if (foreign && !(rate > 0)) {
      error = t("expense.needRate", currency, group.currency);
      return;
    }
    if (amountCents <= 0) {
      error = t("expense.lessThanCent", group.currency);
      return;
    }
    const payer = severalPayers ? payers[0].participant_id : paidBy;
    if (includedParticipants.length === 0) {
      error = t("expense.needPeople");
      return;
    }
    if (splits.some((s) => s.fixed_cents != null && s.fixed_cents <= 0)) {
      error = t("expense.needSetAmounts");
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
              payer,
              together,
              splits,
              createdAt,
              original
            )
          : api.addExpense(
              groupId,
              trimmedTitle,
              amountCents,
              payer,
              together,
              splits,
              createdAt,
              original
            )
      );
      dialogs.expense.open = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }

  const nameWithYou = (p: { id: string; name: string }) =>
    p.id === currentUserId ? t("common.withYou", p.name) : p.name;

  const payerName = $derived.by(() => {
    if (severalPayers) return t("expense.several");
    const p = participants.find((x) => x.id === paidBy);
    return p ? nameWithYou(p) : t("common.choose");
  });
</script>

<Dialog.Root bind:open={dialogs.expense.open}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-lg">
    <Dialog.Header>
      <Dialog.Title>{editing ? t("expense.editTitle") : t("expense.addTitle")}</Dialog.Title>
      <Dialog.Description>
        {editing ? t("expense.editIntro") : t("expense.addIntro")}
      </Dialog.Description>
    </Dialog.Header>

    <form onsubmit={handleSubmit}>
      <Field.Group>
        <Field.Field>
          <Field.Label for="input-expense-title">{t("expense.description")}</Field.Label>
          <Input
            id="input-expense-title"
            required
            bind:value={title}
            placeholder={t("expense.descriptionPlaceholder")}
          />
        </Field.Field>

        <div class="grid grid-cols-2 gap-4">
          <Field.Field>
            <Field.Label for="input-expense-amount">{t("common.amount")}</Field.Label>
            <Input
              id="input-expense-amount"
              type="number"
              inputmode="decimal"
              step="0.01"
              min="0.01"
              required
              readonly={severalPayers}
              bind:value={amountStr}
              placeholder="0.00"
              aria-describedby={severalPayers ? "expense-amount-is-total" : undefined}
              class="tabular-nums read-only:bg-muted"
            />
          </Field.Field>

          <Field.Field>
            <Field.Label for="select-expense-currency">{t("common.currency")}</Field.Label>
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
              <Field.Label for="input-expense-rate">{t("expense.rate")}</Field.Label>
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
                  placeholder={lookingUpRate ? t("expense.lookingUp") : "0.92"}
                  class="tabular-nums"
                />
                <InputGroup.Addon align="inline-end">
                  <InputGroup.Text>{group.currency}</InputGroup.Text>
                </InputGroup.Addon>
              </InputGroup.Root>
              <Field.Description>
                {#if amountCents > 0}
                  {t("expense.countsAs", formatMoney(amountCents, group.currency))}
                {:else}
                  {t("expense.groupCounts", group.currency)}
                {/if}
                {#if !rateIsOwn && rate > 0}
                  {t("expense.suggestedRate")}
                {/if}
              </Field.Description>
            </Field.Field>
          {/if}

          <Field.Field>
            <Field.Label id="label-expense-date" for="input-expense-date"
              >{t("common.date")}</Field.Label
            >
            <DatePicker
              id="input-expense-date"
              labelId="label-expense-date"
              value={expenseDate}
              onChange={chooseDate}
            />
          </Field.Field>

          <Field.Field>
            <Field.Label for="select-expense-payer">{t("common.paidBy")}</Field.Label>
            <Select.Root
              type="single"
              value={severalPayers ? SEVERAL : paidBy}
              onValueChange={choosePayer}
            >
              <Select.Trigger id="select-expense-payer" class="w-full min-w-0">
                <span class="truncate">{payerName}</span>
              </Select.Trigger>
              <Select.Content>
                {#each participants as p (p.id)}
                  <Select.Item value={p.id} label={nameWithYou(p)} />
                {/each}
                {#if participants.length > 1}
                  <Select.Separator />
                  <Select.Item value={SEVERAL} label={t("expense.severalPayers")} />
                {/if}
              </Select.Content>
            </Select.Root>
          </Field.Field>
        </div>

        {#if severalPayers}
          <Field.Set>
            <Field.Legend variant="label" class="mb-0">{t("expense.whoPaid")}</Field.Legend>
            <Field.Description id="expense-amount-is-total">
              {t("expense.whoPaidHelp")}
            </Field.Description>
            <ul class="divide-y rounded-xl border" aria-label={t("expense.whoPaid")}>
              {#each participants as p (p.id)}
                {@const state = payersState[p.id] || { included: false, amount: "" }}
                <li
                  class={cn(
                    "flex min-h-11 items-center justify-between gap-2 py-1.5 pr-1.5 pl-3 sm:pr-3",
                    !state.included && "text-muted-foreground"
                  )}
                >
                  <div class="flex min-w-0 items-center gap-2.5">
                    <Checkbox
                      id={`payer-${p.id}`}
                      checked={state.included}
                      onCheckedChange={() => togglePayer(p.id)}
                      aria-label={t("expense.paidPart", p.name)}
                    />
                    <Label for={`payer-${p.id}`} class="block min-w-0 truncate font-normal">
                      {p.name}
                    </Label>
                  </div>
                  {#if state.included}
                    <Input
                      type="number"
                      inputmode="decimal"
                      step="0.01"
                      min="0.01"
                      bind:value={payersState[p.id].amount}
                      oninput={totalPayers}
                      placeholder="0.00"
                      aria-label={t("expense.paidByAmount", p.name)}
                      class="h-8 w-24 text-right tabular-nums"
                    />
                  {/if}
                </li>
              {/each}
            </ul>
          </Field.Set>
        {/if}

        <Field.Set>
          <div class="flex items-center justify-between">
            <Field.Legend variant="label" class="mb-0">{t("common.splitBetween")}</Field.Legend>
            <Button variant="link" size="sm" onclick={toggleAll}>
              {allIncluded ? t("expense.deselectAll") : t("expense.selectAll")}
            </Button>
          </div>
          <Field.Description>
            {t(
              "expense.people",
              includedParticipants.length,
              participants.length
            )}{#if totalShares > 0},
              {t("common.parts", totalShares)}
              {#if restCents > 0}
                · {t("expense.perPart", formatMoney(Math.floor(restCents / totalShares), currency))}
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
                  "flex min-h-11 items-center justify-between gap-2 py-1.5 pr-1.5 pl-3 sm:pr-3",
                  !state.included && "text-muted-foreground"
                )}
              >
                <div class="flex min-w-0 items-center gap-2.5">
                  <Checkbox
                    id={`split-${p.id}`}
                    checked={state.included}
                    onCheckedChange={() => toggleParticipant(p.id)}
                  />
                  <Label for={`split-${p.id}`} class="block min-w-0 truncate font-normal"
                    >{p.name}</Label
                  >
                </div>

                {#if state.included}
                  <div class="flex shrink-0 items-center gap-1 sm:gap-1.5">
                    {#if state.fixed}
                      <Input
                        type="number"
                        inputmode="decimal"
                        step="0.01"
                        min="0.01"
                        bind:value={splitsState[p.id].amount}
                        placeholder="0.00"
                        aria-label={t("expense.amountFor", p.name)}
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
                          aria-label={t("expense.fewerParts", p.name)}
                        >
                          <MinusIcon />
                        </Button>
                        <span class="min-w-12 text-center text-xs sm:min-w-14" aria-live="polite">
                          {t("common.parts", state.shares)}
                        </span>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          onclick={() => updateShares(p.id, state.shares + 1)}
                          aria-label={t("expense.moreParts", p.name)}
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
                        ? t("expense.giveParts", p.name)
                        : t("expense.setAmountFor", p.name)}
                      title={state.fixed ? t("expense.shareRest") : t("expense.setAmount")}
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
            <Button {...props} variant="outline">{t("common.cancel")}</Button>
          {/snippet}
        </Dialog.Close>
        <Button type="submit" disabled={submitting}>
          {#if submitting}
            <Spinner data-icon="inline-start" />
          {/if}
          {editing ? t("expense.saveChanges") : t("expense.saveNew")}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
