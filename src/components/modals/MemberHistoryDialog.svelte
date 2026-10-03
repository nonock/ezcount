<script lang="ts">
  import UserMinusIcon from "@lucide/svelte/icons/user-minus";
  import UserPlusIcon from "@lucide/svelte/icons/user-plus";
  import UsersIcon from "@lucide/svelte/icons/users";
  import * as Dialog from "@/components/ui/dialog";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { cn } from "@/lib/utils";
  import type { Group } from "@/types";
  import { formatDateTime, formatList } from "@/utils/formatters";

  /** Who was added to the group and removed from it, when and by whom, latest first. */
  let { group }: { group: Group } = $props();

  const nameOf = (id: string | null | undefined) =>
    id ? (group.participants.find((p) => p.id === id)?.name ?? null) : null;

  interface Event {
    key: string;
    at: string;
    added: boolean;
    text: string;
  }

  const events = $derived.by(() => {
    const list: Event[] = [];
    for (const p of group.participants) {
      if (p.added_at) {
        const by = nameOf(p.added_by);
        list.push({
          key: `${p.id}-added`,
          at: p.added_at,
          added: true,
          text:
            p.added_by === p.id
              ? t("members.joined", p.name)
              : by
                ? t("members.addedBy", p.name, by)
                : t("members.added", p.name),
        });
      }
      if (p.removed && p.removed_at) {
        const by = nameOf(p.removed_by);
        list.push({
          key: `${p.id}-removed`,
          at: p.removed_at,
          added: false,
          text: by ? t("members.removedBy", p.name, by) : t("members.removed", p.name),
        });
      }
    }
    return list.sort((a, b) => b.at.localeCompare(a.at));
  });

  // The members the group was made with, and those from before additions were dated.
  const founders = $derived(group.participants.filter((p) => !p.added_at).map((p) => p.name));
</script>

<Dialog.Root bind:open={dialogs.memberHistory}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("members.historyTitle")}</Dialog.Title>
      <Dialog.Description>{t("members.historyIntro")}</Dialog.Description>
    </Dialog.Header>
    <ol class="space-y-3" aria-label={t("members.historyTitle")}>
      {#each events as event (event.key)}
        <li class="flex gap-3">
          <span
            class={cn(
              "flex size-8 shrink-0 items-center justify-center rounded-full",
              event.added ? "bg-positive-soft text-positive" : "bg-muted text-muted-foreground"
            )}
            aria-hidden="true"
          >
            {#if event.added}
              <UserPlusIcon class="size-4" />
            {:else}
              <UserMinusIcon class="size-4" />
            {/if}
          </span>
          <div class="min-w-0">
            <p>{event.text}</p>
            <p class="text-xs text-muted-foreground">{formatDateTime(event.at)}</p>
          </div>
        </li>
      {/each}
      {#if founders.length > 0}
        <li class="flex gap-3">
          <span
            class="flex size-8 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground"
            aria-hidden="true"
          >
            <UsersIcon class="size-4" />
          </span>
          <div class="min-w-0">
            <p>{t("members.founders", formatList(founders), founders.length)}</p>
            <p class="text-xs text-muted-foreground">{formatDateTime(group.created_at)}</p>
          </div>
        </li>
      {/if}
    </ol>
  </Dialog.Content>
</Dialog.Root>
