<script lang="ts">
  import * as Tabs from "@/components/ui/tabs";
  import { t } from "@/lib/i18n/index.svelte";

  export type ExpenseKind = "expense" | "income" | "transfer";

  /**
   * What is being added: an expense, money that came in, or a transfer between two people.
   * The first two share a form and the third has its own, so both show these to go across.
   */
  let { kind, onChange }: { kind: ExpenseKind; onChange: (kind: ExpenseKind) => void } = $props();
</script>

<Tabs.Root value={kind} onValueChange={(value) => onChange(value as ExpenseKind)}>
  <Tabs.List class="w-full" aria-label={t("expense.kind")}>
    <Tabs.Trigger value="expense">{t("expense.kindExpense")}</Tabs.Trigger>
    <Tabs.Trigger value="income">{t("expense.kindIncome")}</Tabs.Trigger>
    <Tabs.Trigger value="transfer">{t("expense.kindTransfer")}</Tabs.Trigger>
  </Tabs.List>
</Tabs.Root>
