<script lang="ts">
  import Amount from "@/components/common/Amount.svelte";
  import { Badge } from "@/components/ui/badge";
  import * as Dialog from "@/components/ui/dialog";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import type { ExpenseSplit, Group, OriginalAmount } from "@/types";
  import { formatDateTime, formatMoney } from "@/utils/formatters";

  let { group }: { group: Group } = $props();

  // Kept after closing, so the dialog doesn't empty while it animates out.
  let expense = $state(dialogs.history);
  $effect.pre(() => {
    if (dialogs.history) expense = dialogs.history;
  });

  const nameOf = (id: string) => group.participants.find((p) => p.id === id)?.name || "Unknown";
  /** "2 parts", or the amount someone owes whatever the others do. */
  function splitLabel(split: ExpenseSplit, original: OriginalAmount | null | undefined) {
    if (split.fixed_cents != null) {
      return formatMoney(split.fixed_cents, original?.currency ?? group.currency);
    }
    return `${split.shares} ${split.shares === 1 ? "part" : "parts"}`;
  }
  const historyEntries = $derived(expense?.history ? [...expense.history].reverse() : []);
</script>

<Dialog.Root
  bind:open={
    () => dialogs.history !== null,
    (open) => {
      if (!open) dialogs.history = null;
    }
  }
>
  {#if expense}
    <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-lg">
      <Dialog.Header>
        <Dialog.Title>Expense Revision History</Dialog.Title>
        <Dialog.Description>
          {historyEntries.length > 0
            ? `Last modified ${formatDateTime(expense.updated_at)}`
            : `Created ${formatDateTime(expense.created_at)}`}
        </Dialog.Description>
      </Dialog.Header>

      <section aria-label="Current version" class="space-y-2 rounded-xl border p-3">
        <div class="flex items-start justify-between gap-2">
          <div>
            <Badge variant="secondary" class="mb-1.5">Current version</Badge>
            <p class="font-medium">{expense.title}</p>
            <p class="text-sm text-muted-foreground">Paid by {nameOf(expense.paid_by)}</p>
          </div>
          <div class="text-right">
            <Amount cents={expense.amount_cents} currency={group.currency} class="font-semibold" />
            {#if expense.original}
              <span class="block text-xs text-muted-foreground tabular-nums">
                {formatMoney(expense.original.amount_cents, expense.original.currency)} at {expense
                  .original.rate}
              </span>
            {/if}
          </div>
        </div>
        <div class="flex flex-wrap gap-1">
          {#each expense.splits as s (s.participant_id)}
            <Badge variant="outline">
              {nameOf(s.participant_id)}
              ({splitLabel(s, expense.original)})
            </Badge>
          {/each}
        </div>
      </section>

      <section class="space-y-3">
        <h3 class="text-sm font-medium text-muted-foreground">
          Change history ({historyEntries.length}
          {historyEntries.length === 1 ? "edit" : "edits"})
        </h3>

        {#if historyEntries.length === 0}
          <p class="rounded-xl border border-dashed p-4 text-center text-sm text-muted-foreground">
            This expense has not been modified since creation.
          </p>
        {:else}
          <ol class="relative space-y-3 border-l pl-5">
            {#each historyEntries as entry, idx (`${entry.edited_at}-${idx}`)}
              <li class="relative">
                <span
                  aria-hidden="true"
                  class="absolute top-1.5 -left-[25px] size-2.5 rounded-full bg-primary ring-4 ring-background"
                ></span>
                <div class="space-y-2 rounded-xl border p-3 text-sm">
                  <div class="flex items-center justify-between gap-2">
                    <span class="font-medium">{formatDateTime(entry.edited_at)}</span>
                    <Badge variant="outline">Revision #{historyEntries.length - idx}</Badge>
                  </div>
                  <p>{entry.summary}</p>
                  <dl
                    class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 rounded-md bg-muted/50 p-2 text-xs"
                  >
                    <dt class="col-span-2 font-medium">Before this edit</dt>
                    <dt class="text-muted-foreground">Title</dt>
                    <dd class="text-right">{entry.previous_title}</dd>
                    <dt class="text-muted-foreground">Amount</dt>
                    <dd class="text-right">
                      <Amount cents={entry.previous_amount_cents} currency={group.currency} />
                      {#if entry.previous_original}
                        ({formatMoney(
                          entry.previous_original.amount_cents,
                          entry.previous_original.currency
                        )} at {entry.previous_original.rate})
                      {/if}
                    </dd>
                    <dt class="text-muted-foreground">Payer</dt>
                    <dd class="text-right">{nameOf(entry.previous_paid_by)}</dd>
                    <dt class="text-muted-foreground">Split</dt>
                    <dd class="text-right">
                      {entry.previous_splits
                        .map(
                          (s) =>
                            `${nameOf(s.participant_id)} (${splitLabel(s, entry.previous_original)})`
                        )
                        .join(", ")}
                    </dd>
                  </dl>
                </div>
              </li>
            {/each}
          </ol>
        {/if}
      </section>
    </Dialog.Content>
  {/if}
</Dialog.Root>
