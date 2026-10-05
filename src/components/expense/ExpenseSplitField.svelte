<script lang="ts">
  import ChartPieIcon from "@lucide/svelte/icons/chart-pie";
  import CoinsIcon from "@lucide/svelte/icons/coins";
  import MinusIcon from "@lucide/svelte/icons/minus";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import { Button } from "@/components/ui/button";
  import { Checkbox } from "@/components/ui/checkbox";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Label } from "@/components/ui/label";
  import type { ExpenseForm } from "@/lib/expenseForm.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { cn } from "@/lib/utils";
  import { formatMoney } from "@/utils/formatters";

  /** Who an expense is split between: each a number of parts, or an amount of their own. */
  let { form }: { form: ExpenseForm } = $props();
</script>

<Field.Set>
  <div class="flex items-center justify-between">
    <Field.Legend variant="label" class="mb-0">{t("common.splitBetween")}</Field.Legend>
    <Button variant="link" size="sm" onclick={() => form.toggleAll()}>
      {form.allIncluded ? t("expense.deselectAll") : t("expense.selectAll")}
    </Button>
  </div>
  <Field.Description>
    {t(
      "expense.people",
      form.includedParticipants.length,
      form.participants.length
    )}{#if form.totalShares > 0},
      {t("common.parts", form.totalShares)}
      {#if form.restCents > 0}
        · {t(
          "expense.perPart",
          formatMoney(Math.floor(form.restCents / form.totalShares), form.currency)
        )}
      {/if}
    {/if}
    {#if form.splitProblem}
      <span class="text-destructive">· {form.splitProblem}</span>
    {/if}
  </Field.Description>

  <ul class="divide-y rounded-xl border">
    {#each form.participants as p (p.id)}
      {@const state = form.splitsState[p.id] || {
        included: false,
        shares: 1,
        fixed: false,
        amount: "",
      }}
      {@const owes = form.owed.get(p.id) ?? 0}
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
            onCheckedChange={() => form.toggleParticipant(p.id)}
          />
          <Label for={`split-${p.id}`} class="block min-w-0 truncate font-normal">{p.name}</Label>
        </div>

        {#if state.included}
          <div class="flex shrink-0 items-center gap-1 sm:gap-1.5">
            {#if state.fixed}
              <Input
                type="number"
                inputmode="decimal"
                step="0.01"
                min="0.01"
                bind:value={form.splitsState[p.id].amount}
                placeholder="0.00"
                aria-label={t("expense.amountFor", p.name)}
                class="h-8 w-24 text-right tabular-nums"
              />
            {:else}
              {#if owes > 0}
                <span class="text-sm tabular-nums">{formatMoney(owes, form.currency)}</span>
              {/if}
              <div class="flex items-center rounded-md border">
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onclick={() => form.updateShares(p.id, state.shares - 1)}
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
                  onclick={() => form.updateShares(p.id, state.shares + 1)}
                  aria-label={t("expense.moreParts", p.name)}
                >
                  <PlusIcon />
                </Button>
              </div>
            {/if}
            <Button
              variant="ghost"
              size="icon-sm"
              onclick={() => (form.splitsState[p.id].fixed = !state.fixed)}
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
