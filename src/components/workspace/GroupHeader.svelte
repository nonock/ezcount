<script lang="ts">
  import DownloadIcon from "@lucide/svelte/icons/download";
  import EllipsisVerticalIcon from "@lucide/svelte/icons/ellipsis-vertical";
  import LogOutIcon from "@lucide/svelte/icons/log-out";
  import PencilIcon from "@lucide/svelte/icons/pencil";
  import UserPlusIcon from "@lucide/svelte/icons/user-plus";
  import UserRoundIcon from "@lucide/svelte/icons/user-round";
  import UsersIcon from "@lucide/svelte/icons/users";
  import Amount from "@/components/common/Amount.svelte";
  import { Badge, badgeVariants } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as Card from "@/components/ui/card";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import { Separator } from "@/components/ui/separator";
  import { exportGroup, leaveGroup } from "@/lib/actions";
  import { backendText } from "@/lib/i18n/backend";
  import { t } from "@/lib/i18n/index.svelte";
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
      {#if group.image}
        <img
          src={group.image}
          alt=""
          class="size-10 shrink-0 rounded-lg object-cover"
          data-testid="group-picture"
        />
      {/if}
      <h1 class="truncate text-2xl font-semibold tracking-tight lg:text-xl lg:whitespace-normal">
        {group.name}
      </h1>
      <Badge variant="soft" class={group.image ? "max-sm:hidden lg:hidden" : "lg:hidden"}
        >{group.currency}</Badge
      >
    </div>
    <Card.Description>
      {t("group.summary", activeParticipants.length)}
      <Amount cents={totalCents} currency={group.currency} class="text-foreground" />
    </Card.Description>
    <Card.Action class="flex items-center gap-1">
      <Button
        variant="outline"
        onclick={() => (dialogs.share = true)}
        title={syncError ? t("share.syncFailed", backendText(syncError)) : t("group.inviteHelp")}
      >
        <!-- The dot tells the sync state; the dialog explains a failure. -->
        <span
          aria-hidden="true"
          class={cn("size-2 rounded-full", syncError ? "bg-negative" : "bg-positive")}
        ></span>
        {t("group.invite")}
        {#if syncError}
          <span class="sr-only">{t("share.syncFailedShort")}</span>
        {/if}
      </Button>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button {...props} variant="ghost" size="icon" aria-label={t("group.options")}>
              <EllipsisVerticalIcon />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end">
          <DropdownMenu.Item onSelect={() => (dialogs.editGroup = true)}>
            <PencilIcon />
            {t("group.edit")}
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => (dialogs.addMember = true)}>
            <UserPlusIcon />
            {t("group.addMember")}
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => (dialogs.who = true)}>
            <UserRoundIcon />
            {t("group.changeWho")}
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={exportGroup}>
            <DownloadIcon />
            {t("group.export")}
          </DropdownMenu.Item>
          <DropdownMenu.Item variant="destructive" onSelect={leaveGroup}>
            <LogOutIcon />
            {t("group.leave")}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </Card.Action>
  </Card.Header>

  <Card.Content class="space-y-4">
    {#if group.description}
      <p class="text-sm whitespace-pre-line">{group.description}</p>
    {/if}
    <ul class="flex flex-wrap gap-1.5" aria-label={t("common.members")}>
      {#each activeParticipants as p (p.id)}
        <li>
          <!-- Renaming and removing a member are both behind their name. -->
          <button
            type="button"
            onclick={() => renameMember(p.id)}
            aria-label={t("group.editMember", p.name)}
            class={cn(
              badgeVariants({ variant: "outline" }),
              "h-7 cursor-pointer gap-1.5 border-transparent px-2.5 text-sm font-normal hover:underline focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
              memberTone(group, p.id)
            )}
          >
            {#if p.avatar}
              <img src={p.avatar} alt="" class="-ml-1.5 size-5 rounded-full object-cover" />
            {/if}
            {p.name}
          </button>
        </li>
      {/each}
      <li>
        <Button variant="ghost" size="sm" onclick={() => (dialogs.addMember = true)} class="h-7">
          <UserPlusIcon data-icon="inline-start" />
          {t("group.addMemberButton")}
        </Button>
      </li>
    </ul>

    {#if activeParticipants.length > 0}
      <Separator />
      {#if me}
        <!-- Where the user stands, their balance first. Who they are is changed from the menu. -->
        <div class="text-sm">
          <dl class="flex flex-wrap items-end gap-x-8 gap-y-2">
            <div>
              <dt class="text-xs text-muted-foreground">{t("group.net")}</dt>
              <dd>
                <Amount
                  cents={currentUserBalance?.net_cents || 0}
                  currency={group.currency}
                  tone="balance"
                  class="text-2xl font-semibold"
                />
              </dd>
            </div>
            <div>
              <dt class="text-xs text-muted-foreground">{t("group.yourExpenses")}</dt>
              <dd>
                <Amount cents={currentUserBalance?.owed_cents || 0} currency={group.currency} />
              </dd>
            </div>
            <div>
              <dt class="text-xs text-muted-foreground">{t("group.paidByYou")}</dt>
              <dd><Amount cents={userPaidCents} currency={group.currency} /></dd>
            </div>
          </dl>
        </div>
      {:else}
        <Button variant="outline" size="sm" onclick={() => (dialogs.who = true)} class="w-fit">
          <UsersIcon data-icon="inline-start" />
          {t("group.whoAreYou")}
        </Button>
      {/if}
    {/if}
  </Card.Content>
</Card.Root>
