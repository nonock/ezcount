<script lang="ts">
  import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
  import SearchIcon from "@lucide/svelte/icons/search";
  import SlidersHorizontalIcon from "@lucide/svelte/icons/sliders-horizontal";
  import Amount from "@/components/common/Amount.svelte";
  import { Button } from "@/components/ui/button";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import * as InputGroup from "@/components/ui/input-group";
  import {
    ANY_CATEGORY,
    ANYONE,
    type ExpenseFilter,
    KINDS,
    type Kind,
    SORTS,
    type Sort,
    spendingCents,
    usedCategories,
  } from "@/lib/expenseList.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { cn } from "@/lib/utils";
  import type { Expense, Group } from "@/types";

  interface Props {
    group: Group;
    filter: ExpenseFilter;
    /** The expenses the filter keeps, to say how many and how much. */
    matching: Expense[];
  }

  /** The expense list's search field and its menu of filters and orders. */
  let { group, filter, matching }: Props = $props();

  const matchingCents = $derived(spendingCents(matching));

  // The menu shows each choice as its current value; opening one lists the others.
  type Section = "sort" | "kind" | "person" | "category";
  let openSection = $state<Section | null>(null);
  const sections = $derived([
    {
      id: "sort" as const,
      label: t("expenses.sort"),
      value: filter.sort as string,
      options: SORTS.map((o) => ({ value: o.value as string, label: t(o.label) })),
    },
    {
      id: "kind" as const,
      label: t("expenses.show"),
      value: filter.kind as string,
      options: KINDS.map((o) => ({ value: o.value as string, label: t(o.label) })),
    },
    {
      id: "person" as const,
      label: t("expenses.involving"),
      value: filter.person,
      options: [
        { value: ANYONE, label: t("expenses.anyone") },
        ...group.participants.map((p) => ({ value: p.id, label: p.name })),
      ],
    },
    {
      id: "category" as const,
      label: t("category.label"),
      value: filter.category,
      options: [{ value: ANY_CATEGORY, label: t("category.any") }, ...usedCategories(group)],
    },
  ]);

  function pick(section: Section, value: string) {
    if (section === "sort") filter.sort = value as Sort;
    else if (section === "kind") filter.kind = value as Kind;
    else if (section === "person") filter.person = value;
    else filter.category = value;
    openSection = null;
  }
</script>

<div class="space-y-2">
  <div class="flex items-center gap-2">
    <InputGroup.Root class="flex-1">
      <InputGroup.Addon>
        <SearchIcon aria-hidden="true" />
      </InputGroup.Addon>
      <InputGroup.Input
        type="search"
        bind:value={filter.query}
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
            {#if filter.narrowed || filter.sort !== "newest"}
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
  {#if filter.filtering}
    <p class="flex flex-wrap items-center gap-x-2 px-1 text-sm text-muted-foreground">
      <span aria-live="polite">
        {t("expenses.count", matching.length)}
        {#if matchingCents > 0}
          · <Amount cents={matchingCents} currency={group.currency} />
        {/if}
      </span>
      <Button variant="link" size="sm" onclick={() => filter.clear()} class="h-auto px-0">
        {t("expenses.clear")}
      </Button>
    </p>
  {/if}
</div>
