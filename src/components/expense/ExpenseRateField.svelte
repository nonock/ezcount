<script lang="ts">
  import * as Field from "@/components/ui/field";
  import * as InputGroup from "@/components/ui/input-group";
  import type { ExpenseForm } from "@/lib/expenseForm.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { formatMoney } from "@/utils/formatters";

  /** The exchange rate of an expense paid in another currency, and what it then counts as. */
  let { form }: { form: ExpenseForm } = $props();
</script>

<Field.Field class="col-span-2">
  <Field.Label for="input-expense-rate">{t("expense.rate")}</Field.Label>
  <InputGroup.Root>
    <InputGroup.Addon>
      <InputGroup.Text>1 {form.currency} =</InputGroup.Text>
    </InputGroup.Addon>
    <InputGroup.Input
      id="input-expense-rate"
      type="number"
      inputmode="decimal"
      step="any"
      min="0"
      required
      bind:value={form.rateStr}
      oninput={() => (form.rateIsOwn = true)}
      placeholder={form.lookingUpRate ? t("expense.lookingUp") : "0.92"}
      class="tabular-nums"
    />
    <InputGroup.Addon align="inline-end">
      <InputGroup.Text>{form.group.currency}</InputGroup.Text>
    </InputGroup.Addon>
  </InputGroup.Root>
  <Field.Description>
    {#if form.amountCents > 0}
      {t("expense.countsAs", formatMoney(form.amountCents, form.group.currency))}
    {:else}
      {t("expense.groupCounts", form.group.currency)}
    {/if}
    {#if !form.rateIsOwn && form.rate > 0}
      {t("expense.suggestedRate")}
    {/if}
  </Field.Description>
</Field.Field>
