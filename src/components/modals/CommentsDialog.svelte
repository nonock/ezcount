<script lang="ts">
  import SendIcon from "@lucide/svelte/icons/send-horizontal";
  import XIcon from "@lucide/svelte/icons/x";
  import Amount from "@/components/common/Amount.svelte";
  import MemberAvatar from "@/components/common/MemberAvatar.svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { api } from "@/services/api";
  import type { Group } from "@/types";
  import { errorMessage } from "@/utils/errors";
  import { expenseTitle, formatDateTime } from "@/utils/formatters";

  /** What the members wrote under an expense, the oldest first, and a field to add to it. */
  let { group }: { group: Group } = $props();

  // Kept after closing, so the dialog doesn't empty while it animates out.
  let expenseId = $state(dialogs.comments);
  $effect.pre(() => {
    if (dialogs.comments) expenseId = dialogs.comments;
  });
  // Read from the group each time, so a comment shows once it is saved or synced.
  const expense = $derived(group.expenses.find((e) => e.id === expenseId));
  const comments = $derived(expense?.comments ?? []);
  const me = $derived(openGroup.currentUserId);

  let text = $state("");
  let sending = $state(false);
  let error = $state<string | null>(null);

  $effect.pre(() => {
    if (dialogs.comments) return;
    text = "";
    error = null;
  });

  const nameOf = (id: string | null | undefined) =>
    (id && group.participants.find((p) => p.id === id)?.name) || t("comments.someone");

  async function send(e: SubmitEvent) {
    e.preventDefault();
    const typed = text.trim();
    if (!expense || !typed) return;
    sending = true;
    error = null;
    try {
      await openGroup.change((groupId) => api.addExpenseComment(groupId, expense.id, typed));
      text = "";
    } catch (err) {
      error = errorMessage(err);
    } finally {
      sending = false;
    }
  }

  async function remove(commentId: string) {
    error = null;
    try {
      await openGroup.change((groupId) => api.deleteExpenseComment(groupId, commentId));
    } catch (err) {
      error = errorMessage(err);
    }
  }
</script>

<Dialog.Root
  bind:open={
    () => dialogs.comments !== null,
    (open) => {
      if (!open) dialogs.comments = null;
    }
  }
>
  {#if expense}
    <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
      <Dialog.Header>
        <Dialog.Title>{t("comments.title")}</Dialog.Title>
        <Dialog.Description>
          {expenseTitle(expense)} ·
          <Amount cents={expense.amount_cents} currency={group.currency} />
        </Dialog.Description>
      </Dialog.Header>

      {#if comments.length === 0}
        <p class="rounded-xl border border-dashed p-4 text-center text-sm text-muted-foreground">
          {t("comments.none")}
        </p>
      {:else}
        <ul class="space-y-3" aria-label={t("comments.title")}>
          {#each comments as comment (comment.id)}
            <li class="flex items-start gap-2.5">
              {#if comment.by}
                <MemberAvatar {group} participantId={comment.by} size="sm" />
              {/if}
              <div class="min-w-0 flex-1">
                <p class="text-xs text-muted-foreground">
                  <span class="font-medium text-foreground">{nameOf(comment.by)}</span>
                  · {formatDateTime(comment.created_at)}
                </p>
                <p class="text-sm break-words whitespace-pre-wrap">{comment.text}</p>
              </div>
              {#if !comment.by || comment.by === me}
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onclick={() => remove(comment.id)}
                  aria-label={t("comments.delete")}
                  class="text-muted-foreground"
                >
                  <XIcon />
                </Button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}

      <form onsubmit={send} class="space-y-2">
        <div class="flex gap-2">
          <Input
            bind:value={text}
            maxlength={500}
            placeholder={t("comments.placeholder")}
            aria-label={t("comments.write")}
            class="min-w-0 flex-1"
          />
          <Button
            type="submit"
            size="icon"
            disabled={sending || !text.trim()}
            aria-label={t("comments.send")}
          >
            <SendIcon />
          </Button>
        </div>
        {#if error}
          <Field.Error>{error}</Field.Error>
        {/if}
      </form>
    </Dialog.Content>
  {/if}
</Dialog.Root>
