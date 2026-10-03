<script lang="ts">
  import Amount from "@/components/common/Amount.svelte";
  import { Badge } from "@/components/ui/badge";
  import * as Dialog from "@/components/ui/dialog";
  import { summaryText } from "@/lib/i18n/backend";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import type { ExpenseSplit, Group, OriginalAmount } from "@/types";
  import { expenseTitle, formatDateTime, formatMoney } from "@/utils/formatters";

  let { group }: { group: Group } = $props();

  // Kept after closing, so the dialog doesn't empty while it animates out.
  let expense = $state(dialogs.history);
  $effect.pre(() => {
    if (dialogs.history) expense = dialogs.history;
  });

  const nameOf = (id: string) =>
    group.participants.find((p) => p.id === id)?.name || t("common.unknown");
  /** "2 parts", or the amount someone owes whatever the others do. */
  function splitLabel(split: ExpenseSplit, original: OriginalAmount | null | undefined) {
    if (split.fixed_cents != null) {
      return formatMoney(split.fixed_cents, original?.currency ?? group.currency);
    }
    return t("common.parts", split.shares);
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
        <Dialog.Title>{t("history.title")}</Dialog.Title>
        <Dialog.Description>
          {historyEntries.length > 0
            ? t("history.modified", formatDateTime(expense.updated_at))
            : t("history.created", formatDateTime(expense.created_at))}
        </Dialog.Description>
      </Dialog.Header>

      <section aria-label={t("history.current")} class="space-y-2 rounded-xl border p-3">
        <div class="flex items-start justify-between gap-2">
          <div>
            <Badge variant="secondary" class="mb-1.5">{t("history.current")}</Badge>
            <p class="font-medium">{expenseTitle(expense)}</p>
            <p class="text-sm text-muted-foreground">
              {t("history.paidBy", nameOf(expense.paid_by))}
            </p>
          </div>
          <div class="text-right">
            <Amount cents={expense.amount_cents} currency={group.currency} class="font-semibold" />
            {#if expense.original}
              <span class="block text-xs text-muted-foreground tabular-nums">
                {t(
                  "history.at",
                  formatMoney(expense.original.amount_cents, expense.original.currency),
                  expense.original.rate
                )}
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
          {t("history.changes", historyEntries.length)}
        </h3>

        {#if historyEntries.length === 0}
          <p class="rounded-xl border border-dashed p-4 text-center text-sm text-muted-foreground">
            {t("history.none")}
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
                    <Badge variant="outline"
                      >{t("history.revision", historyEntries.length - idx)}</Badge
                    >
                  </div>
                  <p>{summaryText(entry.summary)}</p>
                  <dl
                    class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 rounded-md bg-muted/50 p-2 text-xs"
                  >
                    <dt class="col-span-2 font-medium">{t("history.before")}</dt>
                    <dt class="text-muted-foreground">{t("history.titleField")}</dt>
                    <dd class="text-right">{entry.previous_title}</dd>
                    <dt class="text-muted-foreground">{t("common.amount")}</dt>
                    <dd class="text-right">
                      <Amount cents={entry.previous_amount_cents} currency={group.currency} />
                      {#if entry.previous_original}
                        ({t(
                          "history.at",
                          formatMoney(
                            entry.previous_original.amount_cents,
                            entry.previous_original.currency
                          ),
                          entry.previous_original.rate
                        )})
                      {/if}
                    </dd>
                    <dt class="text-muted-foreground">{t("history.payer")}</dt>
                    <dd class="text-right">{nameOf(entry.previous_paid_by)}</dd>
                    <dt class="text-muted-foreground">{t("history.split")}</dt>
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
