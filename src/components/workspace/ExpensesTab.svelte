<script lang="ts">
  import ArrowRightIcon from "@lucide/svelte/icons/arrow-right";
  import CheckIcon from "@lucide/svelte/icons/check";
  import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
  import EllipsisIcon from "@lucide/svelte/icons/ellipsis";
  import HandCoinsIcon from "@lucide/svelte/icons/hand-coins";
  import MessageSquareIcon from "@lucide/svelte/icons/message-square";
  import PencilIcon from "@lucide/svelte/icons/pencil";
  import PiggyBankIcon from "@lucide/svelte/icons/piggy-bank";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import ReceiptTextIcon from "@lucide/svelte/icons/receipt-text";
  import RepeatIcon from "@lucide/svelte/icons/repeat";
  import HistoryIcon from "@lucide/svelte/icons/rotate-ccw-clock";
  import SearchIcon from "@lucide/svelte/icons/search";
  import SearchXIcon from "@lucide/svelte/icons/search-x";
  import SlidersHorizontalIcon from "@lucide/svelte/icons/sliders-horizontal";
  import TrashIcon from "@lucide/svelte/icons/trash";
  import Amount from "@/components/common/Amount.svelte";
  import * as Avatar from "@/components/ui/avatar";
  import { Badge, badgeVariants } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import * as Empty from "@/components/ui/empty";
  import * as InputGroup from "@/components/ui/input-group";
  import * as Item from "@/components/ui/item";
  import { deleteExpense } from "@/lib/actions";
  import { CATEGORIES, categoryName, categoryOf, categoryTone } from "@/lib/categories";
  import { t } from "@/lib/i18n/index.svelte";
  import { paidAmounts, paidCurrency } from "@/lib/split";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { memberTone } from "@/lib/tones";
  import { cn } from "@/lib/utils";
  import type { Expense, Group } from "@/types";
  import {
    expenseTitle,
    formatDateGroupHeader,
    formatList,
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

  /** "Alice", or "Alice and Bob" when several people paid. */
  const payersOf = (e: Expense) => formatList(paidAmounts(e).map((paid) => nameOf(paid.id)));

  // What is shown: a search, a kind, a person, and an order. Kept while the group is open.
  type Kind = "all" | "expenses" | "income" | "payments";
  type Sort = "newest" | "oldest" | "highest" | "lowest" | "title";
  const KINDS = [
    { value: "all", label: "expenses.all" },
    { value: "expenses", label: "expenses.onlyExpenses" },
    { value: "income", label: "expenses.onlyIncome" },
    { value: "payments", label: "expenses.onlyPayments" },
  ] as const;
  const SORTS = [
    { value: "newest", label: "expenses.newest" },
    { value: "oldest", label: "expenses.oldest" },
    { value: "highest", label: "expenses.highest" },
    { value: "lowest", label: "expenses.lowest" },
    { value: "title", label: "expenses.byTitle" },
  ] as const;
  const ANYONE = "";

  let query = $state("");
  let kind = $state<Kind>("all");
  let person = $state(ANYONE);
  const ANY_CATEGORY = "any";
  /** A category's key, "" for the expenses without one, or any. */
  let category = $state(ANY_CATEGORY);
  let sort = $state<Sort>("newest");

  const kindOf = (e: Expense): Kind =>
    e.is_reimbursement ? "payments" : e.income ? "income" : "expenses";
  /** What counts in the totals: payments and money that came in aren't spending. */
  const isSpending = (e: Expense) => kindOf(e) === "expenses";

  const filtering = $derived(
    query.trim() !== "" || kind !== "all" || person !== ANYONE || category !== ANY_CATEGORY
  );
  /** The key the filter and the statistics know an expense's category by. */
  const categoryKey = (e: Expense) =>
    e.is_reimbursement ? null : (categoryOf(e.category)?.key ?? "");
  // Only the categories the group uses are offered.
  const usedCategories = $derived.by(() => {
    const used = new Set(group.expenses.map(categoryKey));
    return [
      ...CATEGORIES.filter((c) => used.has(c.key)).map((c) => ({
        value: c.key as string,
        label: t(c.label),
      })),
      ...(used.has("") ? [{ value: "", label: t("category.none") }] : []),
    ];
  });
  /** By day only makes sense in date order. */
  const byDay = $derived(sort === "newest" || sort === "oldest");

  // The menu shows each choice as its current value; opening one lists the others.
  type Section = "sort" | "kind" | "person" | "category";
  let openSection = $state<Section | null>(null);
  const sections = $derived([
    {
      id: "sort" as const,
      label: t("expenses.sort"),
      value: sort as string,
      options: SORTS.map((o) => ({ value: o.value as string, label: t(o.label) })),
    },
    {
      id: "kind" as const,
      label: t("expenses.show"),
      value: kind as string,
      options: KINDS.map((o) => ({ value: o.value as string, label: t(o.label) })),
    },
    {
      id: "person" as const,
      label: t("expenses.involving"),
      value: person,
      options: [
        { value: ANYONE, label: t("expenses.anyone") },
        ...group.participants.map((p) => ({ value: p.id, label: p.name })),
      ],
    },
    {
      id: "category" as const,
      label: t("category.label"),
      value: category,
      options: [{ value: ANY_CATEGORY, label: t("category.any") }, ...usedCategories],
    },
  ]);

  function pick(section: Section, value: string) {
    if (section === "sort") sort = value as Sort;
    else if (section === "kind") kind = value as Kind;
    else if (section === "person") person = value;
    else category = value;
    openSection = null;
  }

  function clearFilters() {
    query = "";
    kind = "all";
    person = ANYONE;
    category = ANY_CATEGORY;
  }

  /** Lower case without accents, so "cafe" finds "Café". */
  const plain = (text: string) =>
    text
      .normalize("NFD")
      .replace(/\p{Diacritic}/gu, "")
      .toLowerCase();

  // The search looks at the title, at who paid and at the amount as it is shown.
  const matching = $derived.by(() => {
    const words = plain(query).split(/\s+/).filter(Boolean);
    return group.expenses.filter((e) => {
      if (kind !== "all" && kindOf(e) !== kind) return false;
      if (
        person !== ANYONE &&
        !paidAmounts(e).some((paid) => paid.id === person) &&
        !e.splits.some((s) => s.participant_id === person)
      ) {
        return false;
      }
      if (category !== ANY_CATEGORY && categoryKey(e) !== category) return false;
      if (words.length === 0) return true;
      const text = plain(
        `${expenseTitle(e)} ${e.category ? categoryName(e.category) : ""} ${payersOf(e)} ${formatMoney(e.amount_cents, group.currency)} ${(e.amount_cents / 100).toFixed(2)}`
      );
      return words.every((word) => text.includes(word));
    });
  });

  const sortedExpenses = $derived.by(() => {
    const time = (e: Expense) => new Date(e.created_at).getTime() || 0;
    const newest = (a: Expense, b: Expense) => time(b) - time(a);
    const order: Record<Sort, (a: Expense, b: Expense) => number> = {
      newest,
      oldest: (a, b) => -newest(a, b),
      highest: (a, b) => b.amount_cents - a.amount_cents || newest(a, b),
      lowest: (a, b) => a.amount_cents - b.amount_cents || newest(a, b),
      title: (a, b) => expenseTitle(a).localeCompare(expenseTitle(b)) || newest(a, b),
    };
    return [...matching].sort(order[sort]);
  });
  const matchingCents = $derived(
    matching.filter(isSpending).reduce((sum, e) => sum + e.amount_cents, 0)
  );

  // Infinite loading, PAGE_SIZE at a time. GroupPage remakes this tab for each group, so
  // another group starts again at the first page.
  let visibleCount = $state(PAGE_SIZE);
  // Another search or order starts again at the first page.
  $effect(() => {
    void [query, kind, person, category, sort];
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

  // Visible expenses by local calendar day, or all in one list when sorted another way
  const dateGroups = $derived.by(() => {
    const map = new Map<string, DateGroup>();
    for (const exp of visibleExpenses) {
      const key = byDay ? getLocalDateKey(exp.created_at) : "all";
      const existing = map.get(key);
      if (existing) {
        existing.items.push(exp);
        if (isSpending(exp)) existing.totalCents += exp.amount_cents;
      } else {
        map.set(key, {
          dateKey: key,
          displayDate: formatDateGroupHeader(exp.created_at),
          totalCents: isSpending(exp) ? exp.amount_cents : 0,
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
            {#if (group.trash ?? []).length > 0}
              <Badge variant="secondary" class="ml-auto tabular-nums">
                {(group.trash ?? []).length}
              </Badge>
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
  </div>

  {#if group.expenses.length > 0}
    <div class="space-y-2">
      <div class="flex items-center gap-2">
        <InputGroup.Root class="flex-1">
          <InputGroup.Addon>
            <SearchIcon aria-hidden="true" />
          </InputGroup.Addon>
          <InputGroup.Input
            type="search"
            bind:value={query}
            placeholder={t("expenses.search")}
            aria-label={t("expenses.search")}
          />
        </InputGroup.Root>
        <DropdownMenu.Root onOpenChange={() => (openSection = null)}>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button
                {...props}
                variant="outline"
                size="icon"
                aria-label={t("expenses.filterSort")}
                title={t("expenses.filterSort")}
                class="relative"
              >
                <SlidersHorizontalIcon />
                {#if kind !== "all" || person !== ANYONE || category !== ANY_CATEGORY || sort !== "newest"}
                  <span
                    class="absolute -top-1 -right-1 size-2.5 rounded-full bg-primary ring-2 ring-background"
                  >
                    <span class="sr-only">{t("expenses.filtersOn")}</span>
                  </span>
                {/if}
              </Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="max-h-[70dvh] w-64 overflow-y-auto">
            {#each sections as section, i (section.id)}
              {@const open = openSection === section.id}
              {#if i > 0}
                <DropdownMenu.Separator />
              {/if}
              <!-- Like a <details>: the summary says what is chosen, and opens the choices. -->
              <DropdownMenu.Item
                closeOnSelect={false}
                onSelect={() => (openSection = open ? null : section.id)}
                aria-expanded={open}
                class="justify-between gap-3"
              >
                <span class="text-muted-foreground">{section.label}</span>
                <span class="flex min-w-0 items-center gap-1">
                  <span class="truncate">
                    {section.options.find((o) => o.value === section.value)?.label}
                  </span>
                  <ChevronDownIcon
                    class={cn("transition-transform", open && "rotate-180")}
                    aria-hidden="true"
                  />
                </span>
              </DropdownMenu.Item>
              {#if open}
                <DropdownMenu.RadioGroup
                  aria-label={section.label}
                  bind:value={() => section.value, (value) => pick(section.id, value)}
                >
                  {#each section.options as option (option.value)}
                    <DropdownMenu.RadioItem value={option.value} closeOnSelect={false} class="pl-5">
                      {option.label}
                    </DropdownMenu.RadioItem>
                  {/each}
                </DropdownMenu.RadioGroup>
              {/if}
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>
      {#if filtering}
        <p class="flex flex-wrap items-center gap-x-2 px-1 text-sm text-muted-foreground">
          <span aria-live="polite">
            {t("expenses.count", matching.length)}
            {#if matchingCents > 0}
              · <Amount cents={matchingCents} currency={group.currency} />
            {/if}
          </span>
          <Button variant="link" size="sm" onclick={clearFilters} class="h-auto px-0">
            {t("expenses.clear")}
          </Button>
        </p>
      {/if}
    </div>
  {/if}

  {#if group.expenses.length > 0 && matching.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon"><SearchXIcon /></Empty.Media>
        <Empty.Title>{t("expenses.noMatch")}</Empty.Title>
        <Empty.Description>{t("expenses.noMatchHelp")}</Empty.Description>
      </Empty.Header>
      <Empty.Content>
        <Button variant="outline" onclick={clearFilters}>{t("expenses.clear")}</Button>
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
            {byDay
              ? dg.displayDate
              : t(SORTS.find((s) => s.value === sort)?.label ?? SORTS[0].label)}
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
        {#each dg.items as e (e.id)}
          {@const isReimbursement = Boolean(e.is_reimbursement)}
          {@const isIncome = Boolean(e.income)}
          {@const editCount = e.history?.length ?? 0}
          {@const commentCount = e.comments?.length ?? 0}
          {@const totalShares = e.splits.reduce((sum, s) => sum + s.shares, 0)}
          {@const anyFixed = e.splits.some((s) => s.fixed_cents != null)}
          {@const filed = isReimbursement ? null : categoryOf(e.category)}
          <li data-testid="expense-item">
            <Item.Root class="items-start rounded-none sm:items-center">
              <Item.Media>
                <Avatar.Root>
                  <Avatar.Fallback
                    class={isReimbursement || (isIncome && !filed)
                      ? "bg-positive-soft text-positive"
                      : filed
                        ? categoryTone(e.category)
                        : memberTone(group, e.paid_by)}
                    title={filed ? t(filed.label) : undefined}
                  >
                    {#if isReimbursement}
                      <HandCoinsIcon class="size-4" aria-hidden="true" />
                    {:else if filed}
                      <filed.icon class="size-4" aria-hidden="true" />
                      <span class="sr-only">{t(filed.label)}</span>
                    {:else if isIncome}
                      <PiggyBankIcon class="size-4" aria-hidden="true" />
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
                  {#if isIncome}
                    <Badge variant="outline" class="text-positive">{t("expenses.income")}</Badge>
                  {/if}
                  {#if e.recurring}
                    <Badge variant="secondary" title={t("expenses.repeated")}>
                      <RepeatIcon aria-hidden="true" />
                      <span class="sr-only">{t("expenses.repeated")}</span>
                    </Badge>
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
                  {#if commentCount > 0}
                    <button
                      type="button"
                      class={badgeVariants({ variant: "secondary" })}
                      onclick={() => (dialogs.comments = e.id)}
                      aria-label={t("comments.countLabel", commentCount, expenseTitle(e))}
                    >
                      <MessageSquareIcon />
                      {commentCount}
                    </button>
                  {/if}
                </Item.Title>

                {#if isReimbursement}
                  <Item.Description class="flex flex-wrap items-center gap-1">
                    {#if !byDay}
                      <span>{formatDateGroupHeader(e.created_at)} ·</span>
                    {/if}
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
                    {#if !byDay}
                      {formatDateGroupHeader(e.created_at)} ·
                    {/if}
                    {isIncome ? t("common.receivedBy") : t("common.paidBy")}
                    <span class="text-foreground">{payersOf(e)}</span>
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
                    tone={isReimbursement || isIncome ? "positive" : "neutral"}
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
                    <DropdownMenu.Item onSelect={() => (dialogs.comments = e.id)}>
                      <MessageSquareIcon />
                      {t("comments.menu")}
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
