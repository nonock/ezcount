<script lang="ts">
  import * as Field from "@/components/ui/field";
  import * as Select from "@/components/ui/select";
  import { type ExpenseForm, SEVERAL } from "@/lib/expenseForm.svelte";
  import { t } from "@/lib/i18n/index.svelte";

  /** Who paid the expense (or received the money): one person, or several. */
  let { form }: { form: ExpenseForm } = $props();

  const nameWithYou = (p: { id: string; name: string }) =>
    p.id === form.me ? t("common.withYou", p.name) : p.name;

  const payerName = $derived.by(() => {
    if (form.severalPayers) return t("expense.several");
    const p = form.participants.find((x) => x.id === form.paidBy);
    return p ? nameWithYou(p) : t("common.choose");
  });
</script>

<Field.Field>
  <Field.Label for="select-expense-payer">
    {form.income ? t("common.receivedBy") : t("common.paidBy")}
  </Field.Label>
  <Select.Root
    type="single"
    value={form.severalPayers ? SEVERAL : form.paidBy}
    onValueChange={(value) => form.choosePayer(value)}
  >
    <Select.Trigger id="select-expense-payer" class="w-full min-w-0">
      <span class="truncate">{payerName}</span>
    </Select.Trigger>
    <Select.Content>
      {#each form.participants as p (p.id)}
        <Select.Item value={p.id} label={nameWithYou(p)} />
      {/each}
      {#if form.participants.length > 1}
        <Select.Separator />
        <Select.Item value={SEVERAL} label={t("expense.severalPayers")} />
      {/if}
    </Select.Content>
  </Select.Root>
</Field.Field>
