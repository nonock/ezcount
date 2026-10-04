<script lang="ts">
  import RepeatIcon from "@lucide/svelte/icons/repeat";
  import Amount from "@/components/common/Amount.svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Empty from "@/components/ui/empty";
  import { stopRecurring } from "@/lib/actions";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import type { Group, RecurringExpense } from "@/types";
  import { formatDate } from "@/utils/formatters";

  /** The expenses that come back by themselves, with when the next one is due. */
  let { group }: { group: Group } = $props();

  const recurring = $derived(group.recurring ?? []);
  const every = (r: RecurringExpense) =>
    r.every === "week"
      ? t("expense.repeatWeek")
      : r.every === "year"
        ? t("expense.repeatYear")
        : t("expense.repeatMonth");
</script>

<Dialog.Root bind:open={dialogs.recurring}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("recurring.title")}</Dialog.Title>
      <Dialog.Description>{t("recurring.intro")}</Dialog.Description>
    </Dialog.Header>
    {#if recurring.length === 0}
      <Empty.Root class="border border-dashed">
        <Empty.Header>
          <Empty.Media variant="icon"><RepeatIcon /></Empty.Media>
          <Empty.Title>{t("recurring.empty")}</Empty.Title>
          <Empty.Description>{t("recurring.emptyHelp")}</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <ul class="divide-y rounded-xl border" aria-label={t("recurring.title")}>
        {#each recurring as r (r.id)}
          <li class="flex items-center gap-2 py-2 pr-1.5 pl-3">
            <div class="min-w-0 flex-1">
              <p class="flex items-baseline justify-between gap-2">
                <span class="truncate">{r.title}</span>
                <Amount
                  cents={r.amount_cents}
                  currency={group.currency}
                  tone={r.income ? "positive" : "neutral"}
                  class="shrink-0"
                />
              </p>
              <p class="text-xs text-muted-foreground">
                {every(r)}
                {#if r.paused}
                  · <span class="text-destructive">{t("recurring.paused")}</span>
                {:else}
                  · {t("recurring.next", formatDate(r.next))}
                {/if}
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              onclick={() => stopRecurring(r.id)}
              aria-label={t("recurring.stopLabel", r.title)}
            >
              {t("recurring.stop")}
            </Button>
          </li>
        {/each}
      </ul>
    {/if}
  </Dialog.Content>
</Dialog.Root>
