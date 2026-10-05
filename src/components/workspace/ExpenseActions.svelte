<script lang="ts">
  import CheckIcon from "@lucide/svelte/icons/check";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import RepeatIcon from "@lucide/svelte/icons/repeat";
  import HistoryIcon from "@lucide/svelte/icons/rotate-ccw-clock";
  import TrashIcon from "@lucide/svelte/icons/trash";
  import { Badge } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import type { Group } from "@/types";

  /**
   * What the expense list opens: the activity, the repeated expenses and the trash, a payment
   * while someone owes something, and a new expense.
   */
  let { group }: { group: Group } = $props();

  const hasOutstandingDebt = $derived(
    group.expenses.length > 0 && openGroup.settlements.length > 0
  );
  const deleted = $derived((group.trash ?? []).length);
</script>

<div class="flex items-center gap-2">
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button
          {...props}
          variant="outline"
          size="icon"
          aria-label={t("expenses.more")}
          title={t("expenses.more")}
        >
          <HistoryIcon />
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="end">
      <DropdownMenu.Item onSelect={() => (dialogs.activity = true)}>
        <HistoryIcon />
        {t("activity.menu")}
      </DropdownMenu.Item>
      <DropdownMenu.Item onSelect={() => (dialogs.recurring = true)}>
        <RepeatIcon />
        {t("recurring.menu")}
      </DropdownMenu.Item>
      <DropdownMenu.Item onSelect={() => (dialogs.trash = true)}>
        <TrashIcon />
        {t("trash.menu")}
        {#if deleted > 0}
          <Badge variant="secondary" class="ml-auto tabular-nums">{deleted}</Badge>
        {/if}
      </DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
  {#if hasOutstandingDebt}
    <Button variant="outline" onclick={() => dialogs.openReimburse()}>
      <CheckIcon data-icon="inline-start" />
      {t("expenses.reimburse")}
    </Button>
  {/if}
  <!-- On phones, floating above the bottom bar, within reach of the thumb. -->
  <Button
    onclick={() => dialogs.openExpense()}
    class="max-sm:fixed max-sm:right-4 max-sm:bottom-[calc(5rem+env(safe-area-inset-bottom))] max-sm:z-30 max-sm:h-12 max-sm:rounded-full max-sm:px-5 max-sm:text-base max-sm:shadow-lg"
  >
    <PlusIcon data-icon="inline-start" />
    {t("expenses.add")}
  </Button>
</div>
