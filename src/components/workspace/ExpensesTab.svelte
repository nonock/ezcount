<script lang="ts">
  import PlusIcon from "@lucide/svelte/icons/plus";
  import ReceiptTextIcon from "@lucide/svelte/icons/receipt-text";
  import SearchXIcon from "@lucide/svelte/icons/search-x";
  import Amount from "@/components/common/Amount.svelte";
  import { Button } from "@/components/ui/button";
  import * as Empty from "@/components/ui/empty";
  import {
    ExpenseFilter,
    dateGroups as groupByDate,
    sortedExpenses as inOrder,
    matchingExpenses,
    SORTS,
  } from "@/lib/expenseList.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import type { Group } from "@/types";
  import ExpenseActions from "./ExpenseActions.svelte";
  import ExpenseFilters from "./ExpenseFilters.svelte";
  import ExpenseRow from "./ExpenseRow.svelte";

  let { group }: { group: Group } = $props();

  const PAGE_SIZE = 10;

  // Kept while the group is open.
  const filter = new ExpenseFilter();
  const matching = $derived(matchingExpenses(group, filter));
  const sortedExpenses = $derived(inOrder(matching, filter.sort));

  // Infinite loading, PAGE_SIZE at a time. GroupPage remakes this tab for each group, so
  // another group starts again at the first page.
  let visibleCount = $state(PAGE_SIZE);
  // Another search or order starts again at the first page.
  $effect(() => {
    void [filter.query, filter.kind, filter.person, filter.category, filter.sort];
    visibleCount = PAGE_SIZE;
  });

  const visibleExpenses = $derived(sortedExpenses.slice(0, visibleCount));
  const hasMore = $derived(visibleCount < sortedExpenses.length);
  const remainingCount = $derived(sortedExpenses.length - visibleCount);

  function loadMore() {
    visibleCount = Math.min(visibleCount + PAGE_SIZE, sortedExpenses.length);
  }

  let sentinel = $state<HTMLDivElement | null>(null);
  $effect(() => {
    if (!hasMore || !sentinel) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) loadMore();
      },
      { rootMargin: "250px" }
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  const dateGroups = $derived(groupByDate(visibleExpenses, filter.byDay));
</script>

<!-- On phones, room below the list for the floating Add Expense button. -->
<div class="space-y-5 max-sm:pb-16">
  <!-- The count is on the tab and the total in the group's header: only the actions here. -->
  <div class="flex items-center justify-end gap-3">
    <h2 class="sr-only">{t("expenses.heading")}</h2>
    <ExpenseActions {group} />
  </div>

  {#if group.expenses.length > 0}
    <ExpenseFilters {group} {filter} {matching} />
  {/if}

  {#if group.expenses.length > 0 && matching.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon"><SearchXIcon /></Empty.Media>
        <Empty.Title>{t("expenses.noMatch")}</Empty.Title>
        <Empty.Description>{t("expenses.noMatchHelp")}</Empty.Description>
      </Empty.Header>
      <Empty.Content>
        <Button variant="outline" onclick={() => filter.clear()}>{t("expenses.clear")}</Button>
      </Empty.Content>
    </Empty.Root>
  {/if}

  {#if group.expenses.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon"><ReceiptTextIcon /></Empty.Media>
        <Empty.Title>{t("expenses.empty")}</Empty.Title>
        <Empty.Description>
          {t("expenses.emptyHelp")}
        </Empty.Description>
      </Empty.Header>
      <Empty.Content>
        <Button onclick={() => dialogs.openExpense()} class="max-sm:hidden">
          <PlusIcon data-icon="inline-start" />
          {t("expenses.addFirst")}
        </Button>
      </Empty.Content>
    </Empty.Root>
  {/if}

  {#each dateGroups as dg (dg.dateKey)}
    <!-- Days out of view aren't laid out or painted until scrolled to: long lists stay quick. -->
    <section
      aria-labelledby={`date-header-${dg.dateKey}`}
      class="space-y-2 [contain-intrinsic-size:auto_8rem] [content-visibility:auto]"
    >
      <div id={`date-header-${dg.dateKey}`} class="flex items-center justify-between px-1 text-sm">
        <span>
          <span class="font-semibold">
            {filter.byDay
              ? dg.displayDate
              : t(SORTS.find((s) => s.value === filter.sort)?.label ?? SORTS[0].label)}
          </span>
          <span class="text-muted-foreground">
            · {t("expenses.count", dg.items.length)}
          </span>
        </span>
        {#if dg.totalCents > 0}
          <Amount cents={dg.totalCents} currency={group.currency} class="text-muted-foreground" />
        {/if}
      </div>

      <ul class="divide-y overflow-hidden rounded-xl bg-card ring-1 ring-foreground/10">
        {#each dg.items as expense (expense.id)}
          <ExpenseRow {group} {expense} byDay={filter.byDay} />
        {/each}
      </ul>
    </section>
  {/each}

  {#if hasMore}
    <div bind:this={sentinel} class="text-center">
      <Button variant="outline" onclick={loadMore}>
        {t("expenses.loadMore", PAGE_SIZE)}
        <span class="text-muted-foreground">{t("expenses.remaining", remainingCount)}</span>
      </Button>
    </div>
  {/if}

  {#if sortedExpenses.length > PAGE_SIZE}
    <p class="text-center text-xs text-muted-foreground">
      {t("expenses.showing", visibleExpenses.length, sortedExpenses.length)}
    </p>
  {/if}
</div>
