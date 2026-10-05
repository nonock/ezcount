<script lang="ts">
  import ChartColumnIcon from "@lucide/svelte/icons/chart-column";
  import Amount from "@/components/common/Amount.svelte";
  import MemberAvatar from "@/components/common/MemberAvatar.svelte";
  import * as Card from "@/components/ui/card";
  import * as Empty from "@/components/ui/empty";
  import { categoryName, categoryOf, categoryTone } from "@/lib/categories";
  import { i18n, t } from "@/lib/i18n/index.svelte";
  import { byCategory, byMonth, byPerson, incomeCents, spending } from "@/lib/stats";
  import { cn } from "@/lib/utils";
  import type { Group } from "@/types";

  /**
   * What the group spent: in all, by category, by person and by month. Payments don't count,
   * nor does money that came in, which is shown apart.
   */
  let { group }: { group: Group } = $props();

  const expenses = $derived(spending(group));
  const income = $derived(incomeCents(group));
  const totalCents = $derived(expenses.reduce((sum, e) => sum + e.amount_cents, 0));
  const share = (cents: number) => (totalCents > 0 ? (cents / totalCents) * 100 : 0);
  const percent = (cents: number) =>
    new Intl.NumberFormat(i18n.locale, { style: "percent", maximumFractionDigits: 0 }).format(
      share(cents) / 100
    );

  const categories = $derived(byCategory(expenses));
  const people = $derived(byPerson(group, expenses));
  const months = $derived(byMonth(expenses));
</script>

{#snippet bar(width: number, tone = "")}
  <div class="h-2 overflow-hidden rounded-full bg-muted" aria-hidden="true">
    <div
      class={cn("h-full rounded-full", tone ? "bg-(--tone-text)" : "bg-primary", tone)}
      style:width={`${Math.max(width, width > 0 ? 2 : 0)}%`}
    ></div>
  </div>
{/snippet}

{#if expenses.length === 0}
  <Empty.Root class="border border-dashed">
    <Empty.Header>
      <Empty.Media variant="icon"><ChartColumnIcon /></Empty.Media>
      <Empty.Title>{t("stats.empty")}</Empty.Title>
      <Empty.Description>{t("stats.emptyHelp")}</Empty.Description>
    </Empty.Header>
  </Empty.Root>
{:else}
  <div class="space-y-4">
    <h2 class="sr-only">{t("tabs.stats")}</h2>
    <Card.Root size="sm">
      <Card.Content>
        <dl class="flex flex-wrap items-end gap-x-8 gap-y-2 text-sm">
          <div>
            <dt class="text-xs text-muted-foreground">{t("stats.total")}</dt>
            <dd>
              <Amount cents={totalCents} currency={group.currency} class="text-2xl font-semibold" />
            </dd>
          </div>
          <div>
            <dt class="text-xs text-muted-foreground">{t("stats.count")}</dt>
            <dd class="tabular-nums">{expenses.length}</dd>
          </div>
          <div>
            <dt class="text-xs text-muted-foreground">{t("stats.average")}</dt>
            <dd>
              <Amount cents={Math.round(totalCents / expenses.length)} currency={group.currency} />
            </dd>
          </div>
          {#if income > 0}
            <div>
              <dt class="text-xs text-muted-foreground">{t("stats.income")}</dt>
              <dd><Amount cents={income} currency={group.currency} tone="positive" /></dd>
            </div>
          {/if}
        </dl>
      </Card.Content>
    </Card.Root>

    <Card.Root size="sm">
      <Card.Header>
        <Card.Title><h3>{t("stats.byCategory")}</h3></Card.Title>
      </Card.Header>
      <Card.Content>
        <ul class="space-y-3" data-testid="stats-categories">
          {#each categories as row (row.key)}
            {@const category = categoryOf(row.key)}
            <li class="space-y-1.5">
              <div class="flex items-center justify-between gap-3 text-sm">
                <span class="flex min-w-0 items-center gap-2">
                  {#if category}
                    <span
                      class={cn(
                        "flex size-6 shrink-0 items-center justify-center rounded-full",
                        categoryTone(row.key)
                      )}
                      aria-hidden="true"
                    >
                      <category.icon class="size-3.5" />
                    </span>
                  {/if}
                  <span class="truncate">{categoryName(row.key)}</span>
                  <span class="text-xs text-muted-foreground">{t("stats.expenses", row.count)}</span
                  >
                </span>
                <span class="shrink-0 whitespace-nowrap">
                  <Amount cents={row.cents} currency={group.currency} />
                  <span class="text-xs text-muted-foreground tabular-nums">
                    · {percent(row.cents)}
                  </span>
                </span>
              </div>
              {@render bar(share(row.cents), category ? categoryTone(row.key).split(" ")[0] : "")}
            </li>
          {/each}
        </ul>
      </Card.Content>
    </Card.Root>

    <Card.Root size="sm">
      <Card.Header>
        <Card.Title><h3>{t("stats.byPerson")}</h3></Card.Title>
        <Card.Description>{t("stats.byPersonHelp")}</Card.Description>
      </Card.Header>
      <Card.Content>
        <ul class="space-y-3" data-testid="stats-people">
          {#each people as person (person.id)}
            <li class="space-y-1.5">
              <div class="flex items-center justify-between gap-3 text-sm">
                <span class="flex min-w-0 items-center gap-2">
                  <MemberAvatar {group} participantId={person.id} size="sm" />
                  <span class="truncate">{person.name}</span>
                </span>
                <span class="shrink-0 text-right whitespace-nowrap">
                  <Amount cents={person.share} currency={group.currency} />
                  <span class="text-xs text-muted-foreground tabular-nums">
                    · {percent(person.share)}
                  </span>
                </span>
              </div>
              {@render bar(share(person.share))}
              <p class="text-xs text-muted-foreground">
                {t("stats.paid")}
                <Amount cents={person.paid} currency={group.currency} />
              </p>
            </li>
          {/each}
        </ul>
      </Card.Content>
    </Card.Root>

    <Card.Root size="sm">
      <Card.Header>
        <Card.Title><h3>{t("stats.byMonth")}</h3></Card.Title>
      </Card.Header>
      <Card.Content>
        <ul class="space-y-3" data-testid="stats-months">
          {#each months as month (month.key)}
            <li class="space-y-1.5">
              <div class="flex items-center justify-between gap-3 text-sm">
                <span class="first-letter:uppercase">{month.name}</span>
                <Amount cents={month.cents} currency={group.currency} />
              </div>
              {@render bar(month.width)}
            </li>
          {/each}
        </ul>
      </Card.Content>
    </Card.Root>
  </div>
{/if}
