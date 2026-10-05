<script lang="ts">
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import * as Select from "@/components/ui/select";
  import type { ExpenseForm } from "@/lib/expenseForm.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { currencySymbol } from "@/utils/formatters";

  /** What was paid and in which currency. The payers or the lines give the amount once set. */
  let { form }: { form: ExpenseForm } = $props();
</script>

<!-- The amount has the room; the currency is its sign, as narrow as it can be. -->
<div class="col-span-2 flex gap-3">
  <Field.Field class="min-w-0 flex-1">
    <Field.Label for="input-expense-amount">{t("common.amount")}</Field.Label>
    <Input
      id="input-expense-amount"
      type="number"
      inputmode="decimal"
      step="0.01"
      min="0.01"
      required
      readonly={form.severalPayers || form.byItems}
      bind:value={form.amountStr}
      placeholder="0.00"
      aria-describedby={form.severalPayers
        ? "expense-amount-is-total"
        : form.byItems
          ? "expense-amount-is-items"
          : undefined}
      class="tabular-nums read-only:bg-muted"
    />
  </Field.Field>

  <Field.Field class="w-24 shrink-0">
    <Field.Label for="select-expense-currency">{t("common.currency")}</Field.Label>
    <Select.Root
      type="single"
      value={form.currency}
      onValueChange={(code) => form.chooseCurrency(code)}
    >
      <Select.Trigger id="select-expense-currency" class="w-full" title={form.currency}>
        {currencySymbol(form.currency)}
        <span class="sr-only">{form.currency}</span>
      </Select.Trigger>
      <Select.Content align="end" class="min-w-32">
        {#each form.currencies as code (code)}
          {@const symbol = currencySymbol(code)}
          <Select.Item value={code} label={code}>
            {code}
            {#if symbol !== code}
              <span class="text-muted-foreground">{symbol}</span>
            {/if}
          </Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
  </Field.Field>
</div>
