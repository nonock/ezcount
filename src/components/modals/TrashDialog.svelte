<script lang="ts">
  import TrashIcon from "@lucide/svelte/icons/trash";
  import XIcon from "@lucide/svelte/icons/x";
  import Amount from "@/components/common/Amount.svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Empty from "@/components/ui/empty";
  import { purgeExpense, restoreExpense } from "@/lib/actions";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import type { Group } from "@/types";
  import { expenseTitle, formatDateTime } from "@/utils/formatters";

  /** The deleted expenses, the latest first: each can be put back, or removed for good. */
  let { group }: { group: Group } = $props();

  const trash = $derived(group.trash ?? []);
  const nameOf = (id: string | null | undefined) =>
    id ? (group.participants.find((p) => p.id === id)?.name ?? null) : null;
</script>

<Dialog.Root bind:open={dialogs.trash}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("trash.title")}</Dialog.Title>
      <Dialog.Description>{t("trash.intro")}</Dialog.Description>
    </Dialog.Header>
    {#if trash.length === 0}
      <Empty.Root class="border border-dashed">
        <Empty.Header>
          <Empty.Media variant="icon"><TrashIcon /></Empty.Media>
          <Empty.Title>{t("trash.empty")}</Empty.Title>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <ul class="divide-y rounded-xl border" aria-label={t("trash.title")}>
        {#each trash as deleted (deleted.expense.id)}
          {@const title = expenseTitle(deleted.expense)}
          {@const by = nameOf(deleted.deleted_by)}
          {@const when = formatDateTime(deleted.deleted_at)}
          <li class="flex items-center gap-2 py-2 pr-1.5 pl-3">
            <div class="min-w-0 flex-1">
              <p class="flex items-baseline justify-between gap-2">
                <span class="truncate">{title}</span>
                <Amount
                  cents={deleted.expense.amount_cents}
                  currency={group.currency}
                  class="shrink-0"
                />
              </p>
              <p class="text-xs text-muted-foreground">
                {by ? t("trash.deletedBy", by, when) : t("trash.deletedAt", when)}
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              onclick={() => restoreExpense(deleted.expense.id)}
              aria-label={t("trash.restoreLabel", title)}
            >
              {t("trash.restore")}
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              onclick={() => purgeExpense(deleted.expense.id)}
              aria-label={t("trash.purge", title)}
              title={t("trash.purgeConfirm")}
            >
              <XIcon />
            </Button>
          </li>
        {/each}
      </ul>
    {/if}
  </Dialog.Content>
</Dialog.Root>
