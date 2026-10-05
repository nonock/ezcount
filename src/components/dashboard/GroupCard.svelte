<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import Amount from "@/components/common/Amount.svelte";
  import { Badge } from "@/components/ui/badge";
  import * as Card from "@/components/ui/card";
  import { t } from "@/lib/i18n/index.svelte";
  import { spentCents } from "@/lib/split";
  import type { Group } from "@/types";

  interface Props {
    group: Group;
    /** What the group owes the user (above zero) or the user owes it; 0 when unknown. */
    net: number;
    onSelect: (groupId: string) => void;
  }

  /** A group in the list: what it spent, what the user owes or is owed, and a way in. */
  let { group, net, onSelect }: Props = $props();

  const totalCents = $derived(spentCents(group));
  const members = $derived(group.participants.filter((p) => !p.removed).length);
</script>

<li>
  <Card.Root
    class="relative h-full transition-colors has-[button:focus-visible]:ring-3 has-[button:focus-visible]:ring-ring/50 hover:bg-muted/50"
  >
    <Card.Header>
      <Card.Title class="flex items-center gap-2.5">
        {#if group.image}
          <img
            src={group.image}
            alt=""
            width="256"
            height="256"
            class="size-9 shrink-0 rounded-lg object-cover"
            data-testid="group-picture"
          />
        {/if}
        <h2 class="truncate">
          <!-- Stretched over the whole card so any tap on it opens the group. -->
          <button
            type="button"
            onclick={() => onSelect(group.id)}
            class="text-left outline-none after:absolute after:inset-0 after:rounded-xl"
          >
            {group.name}
          </button>
        </h2>
      </Card.Title>
      {#if group.description}
        <Card.Description class="line-clamp-1">{group.description}</Card.Description>
      {/if}
      <Badge variant="soft" class="w-fit">{group.currency}</Badge>
    </Card.Header>
    <Card.Content class="flex items-end justify-between gap-2">
      <div>
        <div class="text-xs text-muted-foreground">{t("groups.totalSpent")}</div>
        <Amount cents={totalCents} currency={group.currency} class="text-lg font-semibold" />
        {#if net !== 0}
          <div class="text-xs" data-testid="group-net">
            <span class="text-muted-foreground">
              {net > 0 ? t("groups.youGetBack") : t("groups.youOwe")}
            </span>
            <Amount
              cents={Math.abs(net)}
              currency={group.currency}
              class={net > 0 ? "text-positive" : "text-negative"}
            />
          </div>
        {/if}
      </div>
      <div class="flex items-center gap-1 text-xs text-muted-foreground">
        {t("groups.summary", members, group.expenses.length)}
        <ChevronRightIcon class="size-4" aria-hidden="true" />
      </div>
    </Card.Content>
  </Card.Root>
</li>
