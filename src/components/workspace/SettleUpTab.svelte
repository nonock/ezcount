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
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import type { Group } from "@/types";

  let { group }: { group: Group } = $props();

  const settlements = $derived(openGroup.settlements);
</script>

<div class="space-y-3">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <div class="flex items-center gap-1">
      <h2 class="font-medium">Suggested payments</h2>
      <HelpPopover title="Optimal settlement plan">
        The fewest direct payments that settle every debt in the group. Mark one as paid once the
        money is sent.
      </HelpPopover>
    </div>
    {#if settlements.length > 0 && group.expenses.length > 0}
      <Button onclick={() => dialogs.openReimburse()}>
        <PlusIcon data-icon="inline-start" />
        Record Reimbursement
      </Button>
    {/if}
  </div>

  {#if settlements.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon" class="text-positive"><CircleCheckBigIcon /></Empty.Media>
        <Empty.Title>All settled up!</Empty.Title>
        <Empty.Description>No one in this group owes anything to anyone.</Empty.Description>
      </Empty.Header>
    </Empty.Root>
  {:else}
    <ul class="space-y-2">
      {#each settlements as s (`${s.from_id}-${s.to_id}`)}
        <li>
          <Item.Root variant="outline">
            <Item.Media>
              <Avatar.Root>
                <Avatar.Fallback>{s.from_name.charAt(0).toUpperCase()}</Avatar.Fallback>
              </Avatar.Root>
            </Item.Media>
            <Item.Content>
              <Item.Title>
                <span>
                  <span class="text-negative">{s.from_name}</span>
                  pays
                  <span class="text-positive">{s.to_name}</span>
                </span>
              </Item.Title>
              <Item.Description>Direct reimbursement</Item.Description>
            </Item.Content>
            <Item.Actions>
              <Amount cents={s.amount_cents} currency={group.currency} class="font-semibold" />
              <Button
                variant="outline"
                onclick={() =>
                  dialogs.openReimburse({
                    fromId: s.from_id,
                    toId: s.to_id,
                    amount: (s.amount_cents / 100).toFixed(2),
                  })}
              >
                <CheckIcon data-icon="inline-start" />
                Mark as Paid
              </Button>
            </Item.Actions>
          </Item.Root>
        </li>
      {/each}
    </ul>
  {/if}
</div>
