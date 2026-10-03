<script lang="ts">
  import ArrowRightIcon from "@lucide/svelte/icons/arrow-right";
  import CheckIcon from "@lucide/svelte/icons/check";
  import EllipsisIcon from "@lucide/svelte/icons/ellipsis";
  import HandCoinsIcon from "@lucide/svelte/icons/hand-coins";
  import PencilIcon from "@lucide/svelte/icons/pencil";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import ReceiptTextIcon from "@lucide/svelte/icons/receipt-text";
  import HistoryIcon from "@lucide/svelte/icons/rotate-ccw-clock";
  import TrashIcon from "@lucide/svelte/icons/trash";
  import Amount from "@/components/common/Amount.svelte";
  import * as Avatar from "@/components/ui/avatar";
  import { Badge, badgeVariants } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import * as Empty from "@/components/ui/empty";
  import * as Item from "@/components/ui/item";
  import { deleteExpense } from "@/lib/actions";
  import { t } from "@/lib/i18n/index.svelte";
  import { paidCurrency } from "@/lib/split";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { memberTone } from "@/lib/tones";
  import type { Expense, Group } from "@/types";
  import {
    expenseTitle,
    formatDateGroupHeader,
    formatMoney,
    getLocalDateKey,
  } from "@/utils/formatters";

  let { group }: { group: Group } = $props();

  const PAGE_SIZE = 10;

  interface DateGroup {
    dateKey: string;
    displayDate: string;
    totalCents: number;
    items: Expense[];
  }

  const names = $derived(new Map(group.participants.map((p) => [p.id, p.name])));
  const nameOf = (id: string | undefined) => (id && names.get(id)) || t("common.unknown");

  const hasOutstandingDebt = $derived(
    group.expenses.length > 0 && openGroup.settlements.length > 0
  );

  // Newest first
  const sortedExpenses = $derived(
    [...group.expenses].sort((a, b) => {
      const timeA = new Date(a.created_at).getTime() || 0;
      const timeB = new Date(b.created_at).getTime() || 0;
      return timeB - timeA;
    })
  );

  // Infinite loading, PAGE_SIZE at a time. GroupPage remakes this tab for each group, so
  // another group starts again at the first page.
  let visibleCount = $state(PAGE_SIZE);

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

  // Visible expenses by local calendar day
  const dateGroups = $derived.by(() => {
    const map = new Map<string, DateGroup>();
    for (const exp of visibleExpenses) {
      const key = getLocalDateKey(exp.created_at);
      const existing = map.get(key);
      if (existing) {
        existing.items.push(exp);
        if (!exp.is_reimbursement) existing.totalCents += exp.amount_cents;
      } else {
        map.set(key, {
          dateKey: key,
          displayDate: formatDateGroupHeader(exp.created_at),
          totalCents: exp.is_reimbursement ? 0 : exp.amount_cents,
          items: [exp],
        });
      }
    }
    return Array.from(map.values());
  });

  // An expense shared equally by everyone is the usual case: only the others list who shares.
  const activeIds = $derived(group.participants.filter((p) => !p.removed).map((p) => p.id));
  const isForEveryone = (e: Expense) =>
    e.splits.length === activeIds.length &&
    e.splits.every(
      (s) =>
        s.fixed_cents == null &&
        s.shares === e.splits[0].shares &&
        activeIds.includes(s.participant_id)
    );
</script>

<!-- On phones, room below the list for the floating Add Expense button. -->
<div class="space-y-5 max-sm:pb-16">
  <!-- The count is on the tab and the total in the group's header: only the actions here. -->
  <div class="flex items-center justify-end gap-3">
    <h2 class="sr-only">{t("expenses.heading")}</h2>
    <div class="flex items-center gap-2">
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
  </div>

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
    <section aria-labelledby={`date-header-${dg.dateKey}`} class="space-y-2">
      <div id={`date-header-${dg.dateKey}`} class="flex items-center justify-between px-1 text-sm">
        <span>
          <span class="font-semibold">{dg.displayDate}</span>
          <span class="text-muted-foreground">
            · {t("expenses.count", dg.items.length)}
          </span>
        </span>
        {#if dg.totalCents > 0}
          <Amount cents={dg.totalCents} currency={group.currency} class="text-muted-foreground" />
        {/if}
      </div>

      <ul class="divide-y overflow-hidden rounded-xl bg-card ring-1 ring-foreground/10">
        {#each dg.items as e (e.id)}
          {@const isReimbursement = Boolean(e.is_reimbursement)}
          {@const editCount = e.history?.length ?? 0}
          {@const totalShares = e.splits.reduce((sum, s) => sum + s.shares, 0)}
          {@const anyFixed = e.splits.some((s) => s.fixed_cents != null)}
          <li data-testid="expense-item">
            <Item.Root class="items-start rounded-none sm:items-center">
              <Item.Media>
                <Avatar.Root>
                  <Avatar.Fallback
                    class={isReimbursement
                      ? "bg-positive-soft text-positive"
                      : memberTone(group, e.paid_by)}
                  >
                    {#if isReimbursement}
                      <HandCoinsIcon class="size-4" aria-hidden="true" />
                    {:else}
                      {e.title.charAt(0).toUpperCase()}
                    {/if}
                  </Avatar.Fallback>
                </Avatar.Root>
              </Item.Media>

              <Item.Content class="min-w-0">
                <Item.Title class="flex-wrap">
                  <h3 class="truncate font-normal">{expenseTitle(e)}</h3>
                  {#if isReimbursement}
                    <Badge variant="outline" class="text-positive"
                      >{t("expenses.reimbursement")}</Badge
                    >
                  {/if}
                  {#if editCount > 0}
                    <button
                      type="button"
                      class={badgeVariants({ variant: "secondary" })}
                      onclick={() => (dialogs.history = e)}
                      aria-label={t("expenses.editedLabel", editCount, expenseTitle(e))}
                    >
                      <HistoryIcon />
                      {t("expenses.edited", editCount)}
                    </button>
                  {/if}
                </Item.Title>

                {#if isReimbursement}
                  <Item.Description class="flex items-center gap-1">
                    <span>{t("common.paidBy")}</span>
                    <span class="text-foreground">{nameOf(e.paid_by)}</span>
                    <ArrowRightIcon class="size-3.5" aria-hidden="true" />
                    <span class="sr-only">{t("expenses.to")}</span>
                    <span class="text-foreground">
                      {nameOf(e.splits[0]?.participant_id)}
                    </span>
                  </Item.Description>
                {:else}
                  <Item.Description>
                    {t("common.paidBy")}
                    <span class="text-foreground">{nameOf(e.paid_by)}</span>
                    {#if isForEveryone(e)}
                      · {t("expenses.forEveryone")}
                    {/if}
                  </Item.Description>
                  {#if !isForEveryone(e)}
                    <ul class="flex flex-wrap gap-1 pt-1" aria-label={t("common.splitBetween")}>
                      {#each e.splits as s (s.participant_id)}
                        <li>
                          <Badge variant="secondary" class="font-normal">
                            {nameOf(s.participant_id)}{s.fixed_cents != null
                              ? ` ${formatMoney(s.fixed_cents, paidCurrency(e, group.currency))}`
                              : s.shares > 1
                                ? ` ×${s.shares}`
                                : ""}
                          </Badge>
                        </li>
                      {/each}
                    </ul>
                  {/if}
                {/if}
              </Item.Content>

              <Item.Actions>
                <div class="text-right">
                  <Amount
                    cents={e.amount_cents}
                    currency={group.currency}
                    tone={isReimbursement ? "positive" : "neutral"}
                    class="block text-base font-semibold"
                  />
                  {#if e.original}
                    <span class="block text-xs text-muted-foreground tabular-nums">
                      {formatMoney(e.original.amount_cents, e.original.currency)}
                    </span>
                  {:else if !isReimbursement && !anyFixed}
                    <span class="block text-xs text-muted-foreground tabular-nums">
                      {formatMoney(Math.floor(e.amount_cents / (totalShares || 1)), group.currency)}
                      {t("expenses.perPart")}
                    </span>
                  {/if}
                </div>
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger>
                    {#snippet child({ props })}
                      <Button
                        {...props}
                        variant="ghost"
                        size="icon"
                        aria-label={t("expenses.actions", expenseTitle(e))}
                      >
                        <EllipsisIcon />
                      </Button>
                    {/snippet}
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="end">
                    <DropdownMenu.Item onSelect={() => dialogs.openExpense(e)}>
                      <PencilIcon />
                      {t("common.edit")}
                    </DropdownMenu.Item>
                    {#if editCount > 0}
                      <DropdownMenu.Item onSelect={() => (dialogs.history = e)}>
                        <HistoryIcon />
                        {t("expenses.viewHistory")}
                      </DropdownMenu.Item>
                    {/if}
                    <DropdownMenu.Separator />
                    <DropdownMenu.Item variant="destructive" onSelect={() => deleteExpense(e.id)}>
                      <TrashIcon />
                      {t("common.delete")}
                    </DropdownMenu.Item>
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
              </Item.Actions>
            </Item.Root>
          </li>
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
