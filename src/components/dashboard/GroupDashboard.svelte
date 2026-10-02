<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import LinkIcon from "@lucide/svelte/icons/link";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import UsersIcon from "@lucide/svelte/icons/users";
  import Amount from "@/components/common/Amount.svelte";
  import { Badge } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as Card from "@/components/ui/card";
  import * as Empty from "@/components/ui/empty";
  import type { Group } from "@/types";

  interface Props {
    groups: Group[];
    onSelectGroup: (groupId: string) => void;
    onOpenCreateGroup: () => void;
    onOpenJoinGroup: () => void;
  }

  let { groups, onSelectGroup, onOpenCreateGroup, onOpenJoinGroup }: Props = $props();
</script>

{#if groups.length === 0}
  <Empty.Root class="border border-dashed py-16">
    <Empty.Header>
      <Empty.Media variant="icon"><UsersIcon /></Empty.Media>
      <Empty.Title>No groups yet</Empty.Title>
      <Empty.Description>
        Create a group for your next trip, dinner, flatshare or event to start splitting bills, or
        join one a friend shared with you.
      </Empty.Description>
    </Empty.Header>
    <Empty.Content class="flex-row justify-center gap-2">
      <Button onclick={onOpenCreateGroup}>
        <PlusIcon data-icon="inline-start" />
        Create Group
      </Button>
      <Button variant="outline" onclick={onOpenJoinGroup}>
        <LinkIcon data-icon="inline-start" />
        Join with Code
      </Button>
    </Empty.Content>
  </Empty.Root>
{:else}
  <div class="space-y-4">
    <div class="flex flex-wrap items-end justify-between gap-2">
      <div>
        <h1 class="text-xl font-semibold tracking-tight">Your Groups</h1>
        <p class="text-sm text-muted-foreground">
          {groups.length}
          active {groups.length === 1 ? "group" : "groups"}
        </p>
      </div>
      <Button variant="outline" onclick={onOpenJoinGroup}>
        <LinkIcon data-icon="inline-start" />
        Join with code
      </Button>
    </div>

    <ul class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {#each groups as group (group.id)}
        {@const totalCents = group.expenses.reduce((sum, e) => sum + e.amount_cents, 0)}
        {@const members = group.participants.filter((p) => !p.removed).length}
        <li>
          <Card.Root
            class="relative transition-colors has-[button:focus-visible]:ring-3 has-[button:focus-visible]:ring-ring/50 hover:bg-muted/50"
          >
            <Card.Header>
              <Card.Title>
                <h2 class="truncate">
                  <!-- Stretched over the whole card so any tap on it opens the group. -->
                  <button
                    type="button"
                    onclick={() => onSelectGroup(group.id)}
                    class="text-left outline-none after:absolute after:inset-0 after:rounded-xl"
                  >
                    {group.name}
                  </button>
                </h2>
              </Card.Title>
              <Badge variant="secondary" class="w-fit">{group.currency}</Badge>
            </Card.Header>
            <Card.Content class="flex items-end justify-between gap-2">
              <div>
                <div class="text-xs text-muted-foreground">Total spent</div>
                <Amount
                  cents={totalCents}
                  currency={group.currency}
                  class="text-lg font-semibold"
                />
              </div>
              <div class="flex items-center gap-1 text-xs text-muted-foreground">
                {members}
                people · {group.expenses.length} records
                <ChevronRightIcon class="size-4" aria-hidden="true" />
              </div>
            </Card.Content>
          </Card.Root>
        </li>
      {/each}
    </ul>
  </div>
{/if}
