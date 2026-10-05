<script lang="ts">
  import ArrowRightIcon from "@lucide/svelte/icons/arrow-right";
  import EllipsisIcon from "@lucide/svelte/icons/ellipsis";
  import HandCoinsIcon from "@lucide/svelte/icons/hand-coins";
  import MessageSquareIcon from "@lucide/svelte/icons/message-square";
  import PencilIcon from "@lucide/svelte/icons/pencil";
  import PiggyBankIcon from "@lucide/svelte/icons/piggy-bank";
  import RepeatIcon from "@lucide/svelte/icons/repeat";
  import HistoryIcon from "@lucide/svelte/icons/rotate-ccw-clock";
  import TrashIcon from "@lucide/svelte/icons/trash";
  import Amount from "@/components/common/Amount.svelte";
  import * as Avatar from "@/components/ui/avatar";
  import { Badge, badgeVariants } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import * as Item from "@/components/ui/item";
  import { deleteExpense } from "@/lib/actions/expenses";
  import { categoryOf, categoryTone } from "@/lib/categories";
  import { isForEveryone, memberName, payersOf } from "@/lib/expenseList.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { paidCurrency } from "@/lib/split";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { memberTone } from "@/lib/tones";
  import type { Expense, Group } from "@/types";
  import { expenseTitle, formatDateGroupHeader, formatMoney } from "@/utils/formatters";

  interface Props {
    group: Group;
    expense: Expense;
    /** Under its day's heading: the row doesn't repeat the date. */
    byDay: boolean;
  }

  /** An expense, a payment or money that came in, as a line of the list, with its menu. */
  let { group, expense: e, byDay }: Props = $props();

  const isReimbursement = $derived(Boolean(e.is_reimbursement));
  const isIncome = $derived(Boolean(e.income));
  const editCount = $derived(e.history?.length ?? 0);
  const commentCount = $derived(e.comments?.length ?? 0);
  const totalShares = $derived(e.splits.reduce((sum, s) => sum + s.shares, 0));
  const anyFixed = $derived(e.splits.some((s) => s.fixed_cents != null));
  const filed = $derived(isReimbursement ? null : categoryOf(e.category));
  const forEveryone = $derived(isForEveryone(group, e));
</script>

<li data-testid="expense-item">
  <Item.Root class="items-start rounded-none sm:items-center">
    <Item.Media>
      <Avatar.Root>
        <Avatar.Fallback
          class={isReimbursement || (isIncome && !filed)
            ? "bg-positive-soft text-positive"
            : filed
              ? categoryTone(e.category)
              : memberTone(group, e.paid_by)}
          title={filed ? t(filed.label) : undefined}
        >
          {#if isReimbursement}
            <HandCoinsIcon class="size-4" aria-hidden="true" />
          {:else if filed}
            <filed.icon class="size-4" aria-hidden="true" />
            <span class="sr-only">{t(filed.label)}</span>
          {:else if isIncome}
            <PiggyBankIcon class="size-4" aria-hidden="true" />
          {:else}
            {e.title.charAt(0).toUpperCase()}
          {/if}
        </Avatar.Fallback>
      </Avatar.Root>
    </Item.Media>

    <Item.Content class="min-w-0">
      <Item.Title class="flex-wrap">
        <h3 class="truncate font-normal">{expenseTitle(e)}</h3>
        {#if isReimbursement}
          <Badge variant="outline" class="text-positive">{t("expenses.reimbursement")}</Badge>
        {/if}
        {#if isIncome}
          <Badge variant="outline" class="text-positive">{t("expenses.income")}</Badge>
        {/if}
        {#if e.recurring}
          <Badge variant="secondary" title={t("expenses.repeated")}>
            <RepeatIcon aria-hidden="true" />
            <span class="sr-only">{t("expenses.repeated")}</span>
          </Badge>
        {/if}
        {#if editCount > 0}
          <button
            type="button"
            class={badgeVariants({ variant: "secondary" })}
            onclick={() => (dialogs.history = e)}
            aria-label={t("expenses.editedLabel", editCount, expenseTitle(e))}
          >
            <HistoryIcon />
            {t("expenses.edited", editCount)}
          </button>
        {/if}
        {#if commentCount > 0}
          <button
            type="button"
            class={badgeVariants({ variant: "secondary" })}
            onclick={() => (dialogs.comments = e.id)}
            aria-label={t("comments.countLabel", commentCount, expenseTitle(e))}
          >
            <MessageSquareIcon />
            {commentCount}
          </button>
        {/if}
      </Item.Title>

      {#if isReimbursement}
        <Item.Description class="flex flex-wrap items-center gap-1">
          {#if !byDay}
            <span>{formatDateGroupHeader(e.created_at)} ·</span>
          {/if}
          <span>{t("common.paidBy")}</span>
          <span class="text-foreground">{memberName(group, e.paid_by)}</span>
          <ArrowRightIcon class="size-3.5" aria-hidden="true" />
          <span class="sr-only">{t("expenses.to")}</span>
          <span class="text-foreground">
            {memberName(group, e.splits[0]?.participant_id)}
          </span>
        </Item.Description>
      {:else}
        <Item.Description>
          {#if !byDay}
            {formatDateGroupHeader(e.created_at)} ·
          {/if}
          {isIncome ? t("common.receivedBy") : t("common.paidBy")}
          <span class="text-foreground">{payersOf(group, e)}</span>
          {#if forEveryone}
            · {t("expenses.forEveryone")}
          {/if}
        </Item.Description>
        {#if !forEveryone}
          <ul class="flex flex-wrap gap-1 pt-1" aria-label={t("common.splitBetween")}>
            {#each e.splits as s (s.participant_id)}
              <li>
                <Badge variant="secondary" class="font-normal">
                  {memberName(group, s.participant_id)}{s.fixed_cents != null
                    ? ` ${formatMoney(s.fixed_cents, paidCurrency(e, group.currency))}`
                    : s.shares > 1
                      ? ` ×${s.shares}`
                      : ""}
                </Badge>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    </Item.Content>

    <Item.Actions>
      <div class="text-right">
        <Amount
          cents={e.amount_cents}
          currency={group.currency}
          tone={isReimbursement || isIncome ? "positive" : "neutral"}
          class="block text-base font-semibold"
        />
        {#if e.original}
          <span class="block text-xs text-muted-foreground tabular-nums">
            {formatMoney(e.original.amount_cents, e.original.currency)}
          </span>
        {:else if !isReimbursement && !anyFixed}
          <span class="block text-xs text-muted-foreground tabular-nums">
            {formatMoney(Math.floor(e.amount_cents / (totalShares || 1)), group.currency)}
            {t("expenses.perPart")}
          </span>
        {/if}
      </div>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              variant="ghost"
              size="icon"
              aria-label={t("expenses.actions", expenseTitle(e))}
            >
              <EllipsisIcon />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end">
          <DropdownMenu.Item onSelect={() => dialogs.openExpense(e)}>
            <PencilIcon />
            {t("common.edit")}
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => (dialogs.comments = e.id)}>
            <MessageSquareIcon />
            {t("comments.menu")}
          </DropdownMenu.Item>
          {#if editCount > 0}
            <DropdownMenu.Item onSelect={() => (dialogs.history = e)}>
              <HistoryIcon />
              {t("expenses.viewHistory")}
            </DropdownMenu.Item>
          {/if}
          <DropdownMenu.Separator />
          <DropdownMenu.Item variant="destructive" onSelect={() => deleteExpense(e.id)}>
            <TrashIcon />
            {t("common.delete")}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </Item.Actions>
  </Item.Root>
</li>
