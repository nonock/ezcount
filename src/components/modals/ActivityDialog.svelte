<script lang="ts">
  import MessageSquareIcon from "@lucide/svelte/icons/message-square";
  import PencilIcon from "@lucide/svelte/icons/pencil";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import RepeatIcon from "@lucide/svelte/icons/repeat";
  import TrashIcon from "@lucide/svelte/icons/trash";
  import UndoIcon from "@lucide/svelte/icons/undo-2";
  import UserMinusIcon from "@lucide/svelte/icons/user-minus";
  import UserPlusIcon from "@lucide/svelte/icons/user-plus";
  import UsersIcon from "@lucide/svelte/icons/users";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Tabs from "@/components/ui/tabs";
  import { type ActivityEvent, groupActivity } from "@/lib/activity";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { cn } from "@/lib/utils";
  import type { Group } from "@/types";
  import { formatDateTime } from "@/utils/formatters";

  /** What happened in the group, the latest first: its expenses, and who came and went. */
  let { group }: { group: Group } = $props();

  const PAGE_SIZE = 30;
  const ICONS = {
    added: PlusIcon,
    repeated: RepeatIcon,
    edited: PencilIcon,
    deleted: TrashIcon,
    restored: UndoIcon,
    commented: MessageSquareIcon,
    joined: UserPlusIcon,
    left: UserMinusIcon,
    created: UsersIcon,
  };
  const TONES: Partial<Record<ActivityEvent["action"], string>> = {
    added: "bg-positive-soft text-positive",
    joined: "bg-positive-soft text-positive",
    deleted: "bg-destructive/10 text-destructive",
  };

  type Show = "all" | ActivityEvent["about"];
  let show = $state<Show>("all");
  let limit = $state(PAGE_SIZE);
  // Each opening starts again from the latest.
  $effect.pre(() => {
    if (dialogs.activity) return;
    show = "all";
    limit = PAGE_SIZE;
  });

  // Only worked out while the dialog is open.
  const events = $derived(dialogs.activity ? groupActivity(group) : []);
  const shown = $derived(events.filter((e) => show === "all" || e.about === show));
</script>

<Dialog.Root bind:open={dialogs.activity}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("activity.title")}</Dialog.Title>
      <Dialog.Description>{t("activity.intro")}</Dialog.Description>
    </Dialog.Header>
    <Tabs.Root
      bind:value={
        () => show,
        (value) => {
          show = value as Show;
          limit = PAGE_SIZE;
        }
      }
    >
      <Tabs.List class="w-full" aria-label={t("activity.filter")}>
        <Tabs.Trigger value="all">{t("activity.all")}</Tabs.Trigger>
        <Tabs.Trigger value="expense">{t("activity.expenses")}</Tabs.Trigger>
        <Tabs.Trigger value="member">{t("activity.members")}</Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>
    <ol class="space-y-3" aria-label={t("activity.title")}>
      {#each shown.slice(0, limit) as event (event.key)}
        {@const Icon = ICONS[event.action]}
        <li class="flex gap-3">
          <span
            class={cn(
              "flex size-8 shrink-0 items-center justify-center rounded-full",
              TONES[event.action] ?? "bg-muted text-muted-foreground"
            )}
            aria-hidden="true"
          >
            <Icon class="size-4" />
          </span>
          <div class="min-w-0">
            <p class="break-words">{event.text}</p>
            <p class="text-xs break-words text-muted-foreground">
              {formatDateTime(event.at)}{event.detail ? ` · ${event.detail}` : ""}
            </p>
          </div>
        </li>
      {/each}
    </ol>
    {#if shown.length > limit}
      <Button variant="outline" onclick={() => (limit += PAGE_SIZE)}>{t("activity.more")}</Button>
    {/if}
  </Dialog.Content>
</Dialog.Root>
