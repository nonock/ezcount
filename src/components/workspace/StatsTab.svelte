<script lang="ts">
  import ChartColumnIcon from "@lucide/svelte/icons/chart-column";
  import Amount from "@/components/common/Amount.svelte";
  import MemberAvatar from "@/components/common/MemberAvatar.svelte";
  import * as Card from "@/components/ui/card";
  import * as Empty from "@/components/ui/empty";
  import { CATEGORIES, categoryName, categoryOf, categoryTone } from "@/lib/categories";
  import { i18n, t } from "@/lib/i18n/index.svelte";
  import { owedAmounts, paidAmounts } from "@/lib/split";
  import { cn } from "@/lib/utils";
  import type { Group } from "@/types";

  /** What the group spent: in all, by category, by person and by month. Payments don't count. */
  let { group }: { group: Group } = $props();

  const expenses = $derived(group.expenses.filter((e) => !e.is_reimbursement));
  const totalCents = $derived(expenses.reduce((sum, e) => sum + e.amount_cents, 0));
  const share = (cents: number) => (totalCents > 0 ? (cents / totalCents) * 100 : 0);
  const percent = (cents: number) =>
    new Intl.NumberFormat(i18n.locale, { style: "percent", maximumFractionDigits: 0 }).format(
      share(cents) / 100
    );

  // Categories in the order they are offered, the biggest first; without a category last.
  const byCategory = $derived.by(() => {
    const totals = new Map<string, { cents: number; count: number }>();
    for (const e of expenses) {
      const key = categoryOf(e.category)?.key ?? "";
      const total = totals.get(key) ?? { cents: 0, count: 0 };
      total.cents += e.amount_cents;
      total.count += 1;
      totals.set(key, total);
    }
    return [...CATEGORIES.map((c) => c.key as string), ""]
      .filter((key) => totals.has(key))
      .map((key) => ({ key, ...(totals.get(key) ?? { cents: 0, count: 0 }) }))
      .sort((a, b) => b.cents - a.cents);
  });

  // Each person's share of the spending, and what they paid.
  const byPerson = $derived.by(() => {
    const totals = new Map(group.participants.map((p) => [p.id, { share: 0, paid: 0 }]));
    const of = (id: string) => {
      let total = totals.get(id);
      if (!total) {
        total = { share: 0, paid: 0 };
        totals.set(id, total);
      }
      return total;
    };
    for (const e of expenses) {
      const owed = owedAmounts(e.amount_cents, e.original?.amount_cents, e.splits);
      e.splits.forEach((s, i) => {
        of(s.participant_id).share += owed[i];
      });
      for (const paid of paidAmounts(e)) of(paid.id).paid += paid.cents;
    }
    return group.participants
      .map((p) => ({ ...p, ...of(p.id) }))
      .filter((p) => !p.removed || p.share > 0 || p.paid > 0)
      .sort((a, b) => b.share - a.share);
  });

  // By calendar month, the latest first.
  const byMonth = $derived.by(() => {
    const totals = new Map<string, number>();
    for (const e of expenses) {
      const date = new Date(e.created_at);
      const key = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
      totals.set(key, (totals.get(key) ?? 0) + e.amount_cents);
    }
    const months = [...totals].sort(([a], [b]) => b.localeCompare(a));
    const most = Math.max(...months.map(([, cents]) => cents), 1);
    return months.map(([key, cents]) => {
      const [year, month] = key.split("-").map(Number);
      const name = new Date(year, month - 1, 1).toLocaleDateString(i18n.locale, {
        month: "long",
        year: "numeric",
      });
      return { key, name, cents, width: (cents / most) * 100 };
    });
  });
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
        </dl>
      </Card.Content>
    </Card.Root>

    <Card.Root size="sm">
      <Card.Header>
        <Card.Title><h3>{t("stats.byCategory")}</h3></Card.Title>
      </Card.Header>
      <Card.Content>
        <ul class="space-y-3" data-testid="stats-categories">
          {#each byCategory as row (row.key)}
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
          {#each byPerson as person (person.id)}
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
          {#each byMonth as month (month.key)}
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
