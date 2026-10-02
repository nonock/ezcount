<script lang="ts">
  import ArrowLeftIcon from "@lucide/svelte/icons/arrow-left";
  import ArrowLeftRightIcon from "@lucide/svelte/icons/arrow-left-right";
  import ReceiptTextIcon from "@lucide/svelte/icons/receipt-text";
  import ScaleIcon from "@lucide/svelte/icons/scale";
  import BottomBar from "@/components/common/BottomBar.svelte";
  import { Badge } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as Tabs from "@/components/ui/tabs";
  import { goHome } from "@/lib/actions";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { navigation } from "@/lib/state/navigation.svelte";
  import { cn } from "@/lib/utils";
  import type { Group, TabType } from "@/types";
  import BalancesTab from "./BalancesTab.svelte";
  import ExpensesTab from "./ExpensesTab.svelte";
  import GroupHeader from "./GroupHeader.svelte";
  import SettleUpTab from "./SettleUpTab.svelte";

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
    Back to All Groups
  </Button>

  <GroupHeader {group} />

  <Tabs.Root bind:value={() => navigation.tab, (tab) => (navigation.tab = tab as TabType)}>
    <BottomBar onHome={goHome}>
      <Tabs.List
        class="w-full sm:w-fit max-sm:h-auto! max-sm:flex-1 max-sm:gap-1 max-sm:bg-transparent max-sm:p-0"
      >
        <Tabs.Trigger value="expenses" class={BOTTOM_TAB}>
          <ReceiptTextIcon />
          Expenses
          <Badge variant="secondary" class={cn("tabular-nums", BOTTOM_TAB_BADGE)}>
            {group.expenses.length}
          </Badge>
        </Tabs.Trigger>
        <Tabs.Trigger value="balances" class={BOTTOM_TAB}>
          <ScaleIcon />
          Balances
        </Tabs.Trigger>
        <Tabs.Trigger value="settle" class={BOTTOM_TAB}>
          <ArrowLeftRightIcon />
          Settle Up
          {#if openGroup.settlements.length > 0}
            <Badge variant="secondary" class={cn("tabular-nums", BOTTOM_TAB_BADGE)}>
              {openGroup.settlements.length}
            </Badge>
          {/if}
        </Tabs.Trigger>
      </Tabs.List>
    </BottomBar>

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
  </Tabs.Root>
</div>
