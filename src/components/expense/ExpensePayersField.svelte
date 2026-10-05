<script lang="ts">
  import { Checkbox } from "@/components/ui/checkbox";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Label } from "@/components/ui/label";
  import type { ExpenseForm } from "@/lib/expenseForm.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { cn } from "@/lib/utils";

  /** The people who paid an expense between them, and how much each. */
  let { form }: { form: ExpenseForm } = $props();
</script>

<Field.Set>
  <Field.Legend variant="label" class="mb-0">{t("expense.whoPaid")}</Field.Legend>
  <Field.Description id="expense-amount-is-total">
    {t("expense.whoPaidHelp")}
  </Field.Description>
  <ul class="divide-y rounded-xl border" aria-label={t("expense.whoPaid")}>
    {#each form.participants as p (p.id)}
      {@const state = form.payersState[p.id] || { included: false, amount: "" }}
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
            onCheckedChange={() => form.togglePayer(p.id)}
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
            bind:value={form.payersState[p.id].amount}
            oninput={() => form.totalPayers()}
            placeholder="0.00"
            aria-label={t("expense.paidByAmount", p.name)}
            class="h-8 w-24 text-right tabular-nums"
          />
        {/if}
      </li>
    {/each}
  </ul>
</Field.Set>
