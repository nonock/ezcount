<script lang="ts">
  import EllipsisVerticalIcon from "@lucide/svelte/icons/ellipsis-vertical";
  import LogOutIcon from "@lucide/svelte/icons/log-out";
  import PencilIcon from "@lucide/svelte/icons/pencil";
  import UserPlusIcon from "@lucide/svelte/icons/user-plus";
  import UserRoundIcon from "@lucide/svelte/icons/user-round";
  import UsersIcon from "@lucide/svelte/icons/users";
  import XIcon from "@lucide/svelte/icons/x";
  import Amount from "@/components/common/Amount.svelte";
  import { Badge } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as Card from "@/components/ui/card";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import { Separator } from "@/components/ui/separator";
  import { leaveGroup, removeMember } from "@/lib/actions";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { memberTone } from "@/lib/tones";
  import { cn } from "@/lib/utils";
  import type { Group } from "@/types";

  let { group }: { group: Group } = $props();

  const activeParticipants = $derived(group.participants.filter((p) => !p.removed));
  const totalCents = $derived(group.expenses.reduce((sum, e) => sum + e.amount_cents, 0));
  const currentUserId = $derived(openGroup.currentUserId);
  const currentUserBalance = $derived(
    openGroup.balances.find((b) => b.participant_id === currentUserId)
  );
  const userPaidCents = $derived(
    group.expenses
      .filter((e) => e.paid_by === currentUserId && !e.is_reimbursement)
      .reduce((sum, e) => sum + e.amount_cents, 0)
  );
  const me = $derived(group.participants.find((p) => p.id === currentUserId));
  const syncError = $derived(openGroup.syncInfo?.last_error ?? null);

  function renameMember(participantId: string) {
    const member = group.participants.find((p) => p.id === participantId);
    if (member) dialogs.renameMember = { open: true, member };
  }
</script>

<Card.Root>
  <Card.Header>
    <div class="flex min-w-0 items-center gap-2">
      <h1 class="truncate text-lg font-semibold tracking-tight">{group.name}</h1>
      <Badge variant="soft">{group.currency}</Badge>
    </div>
    <Card.Description>
      {activeParticipants.length}
      participants · Group total
      <Amount cents={totalCents} currency={group.currency} class="text-foreground" />
    </Card.Description>
    <Card.Action class="flex items-center gap-1">
      <Button
        variant="outline"
        onclick={() => (dialogs.share = true)}
        title={syncError ? `Last sync failed: ${syncError}` : "Invite other members"}
      >
        <!-- The dot tells the sync state; the dialog explains a failure. -->
        <span
          aria-hidden="true"
          class={cn("size-2 rounded-full", syncError ? "bg-negative" : "bg-positive")}
        ></span>
        Invite
        {#if syncError}
          <span class="sr-only">(last sync failed)</span>
        {/if}
      </Button>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button {...props} variant="ghost" size="icon" aria-label="Group options">
              <EllipsisVerticalIcon />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end">
          <DropdownMenu.Item onSelect={() => (dialogs.editGroup = true)}>
            <PencilIcon />
            Edit group
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => (dialogs.addMember = true)}>
            <UserPlusIcon />
            Add member
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => (dialogs.who = true)}>
            <UserRoundIcon />
            Change who you are
          </DropdownMenu.Item>
          <DropdownMenu.Item variant="destructive" onSelect={leaveGroup}>
            <LogOutIcon />
            Leave group
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </Card.Action>
  </Card.Header>

  <Card.Content class="space-y-4">
    <ul class="flex flex-wrap gap-1.5" aria-label="Members">
      {#each activeParticipants as p (p.id)}
        <li>
          <Badge
            variant="outline"
            class={cn(
              "h-7 gap-1 border-transparent pr-0.5 pl-2.5 text-sm",
              memberTone(group, p.id)
            )}
          >
            <button
              type="button"
              onclick={() => renameMember(p.id)}
              aria-label={`Rename ${p.name}`}
              title="Rename"
              class="cursor-pointer rounded-sm hover:underline focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
            >
              {p.name}
            </button>
            <Button
              variant="ghost"
              size="icon-xs"
              onclick={() => removeMember(p.id)}
              aria-label={`Remove ${p.name}`}
              class="text-muted-foreground hover:text-destructive"
            >
              <XIcon />
            </Button>
          </Badge>
        </li>
      {/each}
      <li>
        <Button variant="ghost" size="sm" onclick={() => (dialogs.addMember = true)} class="h-7">
          <UserPlusIcon data-icon="inline-start" />
          Add Member
        </Button>
      </li>
    </ul>

    {#if activeParticipants.length > 0}
      <Separator />
      <div class="flex flex-col gap-3 text-sm md:flex-row md:items-center md:justify-between">
        {#if me}
          <p class="flex items-center gap-1 text-muted-foreground">
            You're <span class="font-medium text-foreground">{me.name}</span>
            <Button
              variant="link"
              size="sm"
              onclick={() => (dialogs.who = true)}
              class="h-auto px-1"
            >
              Change
            </Button>
          </p>
        {:else}
          <Button variant="outline" size="sm" onclick={() => (dialogs.who = true)} class="w-fit">
            <UsersIcon data-icon="inline-start" />
            Who are you in this group?
          </Button>
        {/if}

        <dl class="grid grid-cols-3 gap-3 md:flex md:gap-6">
          <div>
            <dt class="text-xs text-muted-foreground">Your expenses</dt>
            <dd>
              <Amount
                cents={currentUserBalance?.owed_cents || 0}
                currency={group.currency}
                class="font-medium"
              />
            </dd>
          </div>
          <div>
            <dt class="text-xs text-muted-foreground">Paid by you</dt>
            <dd>
              <Amount cents={userPaidCents} currency={group.currency} class="font-medium" />
            </dd>
          </div>
          <div>
            <dt class="text-xs text-muted-foreground">Net</dt>
            <dd>
              <Amount
                cents={currentUserBalance?.net_cents || 0}
                currency={group.currency}
                tone="balance"
                class="font-semibold"
              />
            </dd>
          </div>
        </dl>
      </div>
    {/if}
  </Card.Content>
</Card.Root>
