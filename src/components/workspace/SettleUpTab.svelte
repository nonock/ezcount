<script lang="ts">
  import CheckIcon from "@lucide/svelte/icons/check";
  import CircleCheckBigIcon from "@lucide/svelte/icons/circle-check-big";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import Amount from "@/components/common/Amount.svelte";
  import HelpPopover from "@/components/common/HelpPopover.svelte";
  import * as Avatar from "@/components/ui/avatar";
  import { Button } from "@/components/ui/button";
  import * as Empty from "@/components/ui/empty";
  import * as Item from "@/components/ui/item";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { memberTone } from "@/lib/tones";
  import type { Group } from "@/types";

  let { group }: { group: Group } = $props();

  const settlements = $derived(openGroup.settlements);
</script>

<div class="space-y-3">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <div class="flex items-center gap-1">
      <h2 class="text-base font-semibold">{t("settle.heading")}</h2>
      <HelpPopover title={t("settle.helpTitle")}>{t("settle.help")}</HelpPopover>
    </div>
    {#if settlements.length > 0 && group.expenses.length > 0}
      <Button variant="ghost" onclick={() => dialogs.openReimburse()}>
        <PlusIcon data-icon="inline-start" />
        {t("settle.record")}
      </Button>
    {/if}
  </div>

  {#if settlements.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon" class="text-positive"><CircleCheckBigIcon /></Empty.Media>
        <Empty.Title>{t("settle.allSettled")}</Empty.Title>
        <Empty.Description>{t("settle.nobodyOwes")}</Empty.Description>
      </Empty.Header>
    </Empty.Root>
  {:else}
    <ul class="divide-y overflow-hidden rounded-xl bg-card ring-1 ring-foreground/10">
      {#each settlements as s (`${s.from_id}-${s.to_id}`)}
        <li>
          <Item.Root class="rounded-none">
            <Item.Media>
              <Avatar.Root>
                <Avatar.Fallback class={memberTone(group, s.from_id)}
                  >{s.from_name.charAt(0).toUpperCase()}</Avatar.Fallback
                >
              </Avatar.Root>
            </Item.Media>
            <Item.Content>
              <Item.Title>
                <span>
                  <span class="text-negative">{s.from_name}</span>
                  {t("settle.pays")}
                  <span class="text-positive">{s.to_name}</span>
                </span>
              </Item.Title>
              <Item.Description>{t("settle.direct")}</Item.Description>
            </Item.Content>
            <Item.Actions>
              <Amount
                cents={s.amount_cents}
                currency={group.currency}
                class="text-base font-semibold"
              />
              <Button
                onclick={() =>
                  dialogs.openReimburse({
                    fromId: s.from_id,
                    toId: s.to_id,
                    amount: (s.amount_cents / 100).toFixed(2),
                  })}
              >
                <CheckIcon data-icon="inline-start" />
                {t("settle.markPaid")}
              </Button>
            </Item.Actions>
          </Item.Root>
        </li>
      {/each}
    </ul>
  {/if}
</div>
