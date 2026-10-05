<script lang="ts">
  import ListIcon from "@lucide/svelte/icons/list";
  import UsersIcon from "@lucide/svelte/icons/users";
  import { untrack } from "svelte";
  import DatePicker from "@/components/common/DatePicker.svelte";
  import ExpenseAmountField from "@/components/expense/ExpenseAmountField.svelte";
  import ExpenseCategoryField from "@/components/expense/ExpenseCategoryField.svelte";
  import ExpenseItemsField from "@/components/expense/ExpenseItemsField.svelte";
  import ExpenseKindTabs, { type ExpenseKind } from "@/components/expense/ExpenseKindTabs.svelte";
  import ExpensePayerField from "@/components/expense/ExpensePayerField.svelte";
  import ExpensePayersField from "@/components/expense/ExpensePayersField.svelte";
  import ExpenseRateField from "@/components/expense/ExpenseRateField.svelte";
  import ExpenseRepeatMenu from "@/components/expense/ExpenseRepeatMenu.svelte";
  import ExpenseSplitField from "@/components/expense/ExpenseSplitField.svelte";
  import { Badge } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import { ExpenseForm } from "@/lib/expenseForm.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { api } from "@/services/api";
  import type { Group } from "@/types";
  import { errorMessage } from "@/utils/errors";
  import { formatMoney } from "@/utils/formatters";
  import ReimbursementForm from "./ReimbursementForm.svelte";

  /**
   * Adds an expense, or money that came in, or edits `dialogs.expense.editing`. A transfer
   * between two people is a payment: its form takes this one's place, in the same dialog.
   */
  let { group }: { group: Group } = $props();

  const editing = $derived(dialogs.expense.editing);
  const form = new ExpenseForm({
    group: () => group,
    editing: () => editing,
    me: () => openGroup.currentUserId,
  });

  // A transfer shows the payment form instead, which is shorter. The dialog is centered, so
  // it would move up to shrink: its top is held where it was, and the tabs stay under the
  // finger.
  let transfer = $state(false);
  let content = $state<HTMLElement | null>(null);
  let heldTop = $state<number | null>(null);
  // Set on the element: the dialog component keeps the style attribute for itself.
  $effect(() => {
    const dialog = content;
    if (!dialog) return;
    // At once, both ways: the dialog animates what moves it, and would slide.
    dialog.style.setProperty("transition", "none");
    dialog.style.setProperty("top", heldTop === null ? "" : `${heldTop}px`);
    dialog.style.setProperty("--tw-translate-y", heldTop === null ? "" : "0");
    if (heldTop !== null) return;
    const frame = requestAnimationFrame(() => dialog.style.setProperty("transition", ""));
    return () => cancelAnimationFrame(frame);
  });
  let submitting = $state(false);
  let error = $state<string | null>(null);

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
      transfer = false;
      heldTop = null;
      form.reset();
    });
  });

  function chooseKind(kind: ExpenseKind) {
    if (kind === "transfer") {
      heldTop = content?.getBoundingClientRect().top ?? null;
      transfer = true;
      return;
    }
    transfer = false;
    heldTop = null;
    form.income = kind === "income";
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const result = form.result();
    if (result.error !== undefined) {
      error = result.error;
      return;
    }
    const { input } = result;
    submitting = true;
    error = null;
    const expense = editing;
    try {
      await openGroup.change((groupId) =>
        expense ? api.updateExpense(groupId, expense.id, input) : api.addExpense(groupId, input)
      );
      dialogs.expense.open = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog.Root bind:open={dialogs.expense.open}>
  <Dialog.Content bind:ref={content} class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-lg">
    <Dialog.Header>
      <Dialog.Title>
        {#if transfer}
          {t("reimburse.title")}
        {:else if form.income}
          {editing ? t("expense.editIncomeTitle") : t("expense.addIncomeTitle")}
        {:else}
          {editing ? t("expense.editTitle") : t("expense.addTitle")}
        {/if}
      </Dialog.Title>
      <Dialog.Description>
        {editing
          ? t("expense.editIntro")
          : transfer
            ? t("expense.transferIntro")
            : form.income
              ? t("expense.addIncomeIntro")
              : t("expense.addIntro")}
      </Dialog.Description>
    </Dialog.Header>

    {#if transfer}
      <ReimbursementForm {group} onDone={() => (dialogs.expense.open = false)}>
        <ExpenseKindTabs kind="transfer" onChange={chooseKind} />
      </ReimbursementForm>
    {:else}
      <form onsubmit={handleSubmit}>
        <Field.Group>
          {#if !editing}
            <ExpenseKindTabs kind={form.income ? "income" : "expense"} onChange={chooseKind} />
          {/if}

          <Field.Field>
            <Field.Label for="input-expense-title">{t("expense.description")}</Field.Label>
            <Input
              id="input-expense-title"
              required
              bind:value={form.title}
              placeholder={form.income
                ? t("expense.incomePlaceholder")
                : t("expense.descriptionPlaceholder")}
            />
          </Field.Field>

          <ExpenseCategoryField bind:value={form.category} />

          <div class="grid grid-cols-2 gap-4">
            <ExpenseAmountField {form} />

            {#if form.foreign}
              <ExpenseRateField {form} />
            {/if}

            <Field.Field>
              <Field.Label id="label-expense-date" for="input-expense-date"
                >{t("common.date")}</Field.Label
              >
              <DatePicker
                id="input-expense-date"
                labelId="label-expense-date"
                value={form.expenseDate}
                onChange={(day) => form.chooseDate(day)}
              />
            </Field.Field>

            <ExpensePayerField {form} />

            {#if !editing && !form.foreign}
              <ExpenseRepeatMenu bind:value={form.repeat} />
            {/if}
          </div>

          {#if form.severalPayers}
            <ExpensePayersField {form} />
          {/if}

          {#if form.byItems}
            <Field.Set>
              <Field.Legend variant="label" class="mb-0">{t("expense.items")}</Field.Legend>
              <Field.Description id="expense-amount-is-items">
                {t("expense.itemsHelp")}
                {#if form.severalPayers && form.itemsCents !== form.paidCents}
                  <span class="text-destructive">· {form.itemsMismatch}</span>
                {/if}
              </Field.Description>
              <ExpenseItemsField
                bind:items={form.itemsState}
                participants={form.participants}
                onChange={() => form.totalItems()}
              />
              {#if form.itemsOwe.size > 0}
                <ul class="flex flex-wrap gap-1" aria-label={t("expense.itemsOwed")}>
                  {#each form.itemsOwe as [id, cents] (id)}
                    <li>
                      <Badge variant="secondary" class="font-normal">
                        {form.participants.find((p) => p.id === id)?.name}
                        {formatMoney(cents, form.currency)}
                      </Badge>
                    </li>
                  {/each}
                </ul>
              {/if}
            </Field.Set>
          {:else}
            <ExpenseSplitField {form} />
          {/if}

          <!-- Seldom used: a small button, as for repeating. -->
          <Button
            variant="ghost"
            size="sm"
            onclick={() => form.toggleItems()}
            class="-mt-3 -ml-2 h-7 w-fit font-normal text-muted-foreground"
          >
            {#if form.byItems}
              <UsersIcon data-icon="inline-start" />
              {t("expense.byPeople")}
            {:else}
              <ListIcon data-icon="inline-start" />
              {t("expense.byItems")}
            {/if}
          </Button>

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
            {editing
              ? t("expense.saveChanges")
              : form.income
                ? t("expense.saveIncome")
                : t("expense.saveNew")}
          </Button>
        </Dialog.Footer>
      </form>
    {/if}
  </Dialog.Content>
</Dialog.Root>
