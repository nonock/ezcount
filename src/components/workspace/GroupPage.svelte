<script lang="ts">
  import ArrowLeftIcon from "@lucide/svelte/icons/arrow-left";
  import ArrowLeftRightIcon from "@lucide/svelte/icons/arrow-left-right";
  import ChartColumnIcon from "@lucide/svelte/icons/chart-column";
  import ReceiptTextIcon from "@lucide/svelte/icons/receipt-text";
  import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
  import ScaleIcon from "@lucide/svelte/icons/scale";
  import BottomBar from "@/components/common/BottomBar.svelte";
  import { Badge } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as Tabs from "@/components/ui/tabs";
  import { goHome } from "@/lib/actions";
  import { t } from "@/lib/i18n/index.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { navigation } from "@/lib/state/navigation.svelte";
  import { cn } from "@/lib/utils";
  import type { Group, TabType } from "@/types";
  import BalancesTab from "./BalancesTab.svelte";
  import DeletionRequest from "./DeletionRequest.svelte";
  import ExpensesTab from "./ExpensesTab.svelte";
  import GroupHeader from "./GroupHeader.svelte";
  import SettleUpTab from "./SettleUpTab.svelte";
  import StatsTab from "./StatsTab.svelte";

  let { group }: { group: Group } = $props();

  /** On phones, in the bottom bar: icon above label, the active tab tinted rather than raised. */
  const BOTTOM_TAB =
    "max-sm:h-auto! max-sm:flex-col max-sm:gap-0.5 max-sm:py-1.5 max-sm:text-xs max-sm:data-active:border-transparent! max-sm:data-active:bg-muted! max-sm:data-active:shadow-none! max-sm:[&_svg]:size-5!";

  /** On phones: a counter on the icon's corner, like a notification. */
  const BOTTOM_TAB_BADGE =
    "max-sm:absolute max-sm:top-0.5 max-sm:left-1/2 max-sm:ml-1.5 max-sm:h-4 max-sm:min-w-4 max-sm:px-1 max-sm:text-[0.625rem]";
</script>

<div class="space-y-4">
  <Button variant="ghost" size="sm" onclick={goHome} class="-ml-2 max-sm:hidden">
    <ArrowLeftIcon data-icon="inline-start" />
    {t("group.back")}
  </Button>

  <!-- On wide screens the group stays on the left while its tabs scroll on the right. -->
  <div
    class="space-y-4 lg:grid lg:grid-cols-[22rem_minmax(0,1fr)] lg:items-start lg:gap-6 lg:space-y-0"
  >
    <div class="space-y-4 lg:sticky lg:top-20">
      <GroupHeader {group} />
      <DeletionRequest {group} />
    </div>

    <Tabs.Root bind:value={() => navigation.tab, (tab) => (navigation.tab = tab as TabType)}>
      <BottomBar onHome={goHome}>
        <Tabs.List
          class="w-full sm:w-fit max-sm:h-auto! max-sm:flex-1 max-sm:gap-1 max-sm:bg-transparent max-sm:p-0"
        >
          <Tabs.Trigger value="expenses" class={BOTTOM_TAB}>
            <ReceiptTextIcon />
            {t("tabs.expenses")}
            <Badge variant="secondary" class={cn("tabular-nums", BOTTOM_TAB_BADGE)}>
              {group.expenses.length}
            </Badge>
          </Tabs.Trigger>
          <Tabs.Trigger value="balances" class={BOTTOM_TAB}>
            <ScaleIcon />
            {t("tabs.balances")}
          </Tabs.Trigger>
          <Tabs.Trigger value="settle" class={BOTTOM_TAB}>
            <ArrowLeftRightIcon />
            {t("tabs.settle")}
            {#if openGroup.settlements.length > 0}
              <Badge variant="secondary" class={cn("tabular-nums", BOTTOM_TAB_BADGE)}>
                {openGroup.settlements.length}
              </Badge>
            {/if}
          </Tabs.Trigger>
          <Tabs.Trigger value="stats" class={BOTTOM_TAB}>
            <ChartColumnIcon />
            {t("tabs.stats")}
          </Tabs.Trigger>
        </Tabs.List>
      </BottomBar>

      {#if openGroup.stale}
        <!-- Other devices changed the group: shown when asked, so nothing moves by itself. -->
        <div class="pt-4" role="status">
          <Button
            variant="outline"
            class="w-full border-primary/40 text-primary"
            onclick={() => openGroup.load(group.id)}
          >
            <RefreshCwIcon data-icon="inline-start" />
            {t("group.refresh")}
          </Button>
        </div>
      {/if}

      <!-- Only the open tab is rendered, so the others' amounts and buttons aren't in the page. -->
      <Tabs.Content value="expenses" class="pt-4">
        {#if navigation.tab === "expenses"}
          {#key group.id}
            <ExpensesTab {group} />
          {/key}
        {/if}
      </Tabs.Content>
      <Tabs.Content value="balances" class="pt-4">
        {#if navigation.tab === "balances"}
          <BalancesTab {group} />
        {/if}
      </Tabs.Content>
      <Tabs.Content value="settle" class="pt-4">
        {#if navigation.tab === "settle"}
          <SettleUpTab {group} />
        {/if}
      </Tabs.Content>
      <Tabs.Content value="stats" class="pt-4">
        {#if navigation.tab === "stats"}
          <StatsTab {group} />
        {/if}
      </Tabs.Content>
    </Tabs.Root>
  </div>
</div>
