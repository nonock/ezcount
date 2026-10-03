<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import FileUpIcon from "@lucide/svelte/icons/file-up";
  import LinkIcon from "@lucide/svelte/icons/link";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import UsersIcon from "@lucide/svelte/icons/users";
  import Amount from "@/components/common/Amount.svelte";
  import { Badge } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as Card from "@/components/ui/card";
  import * as Empty from "@/components/ui/empty";
  import { importGroup } from "@/lib/actions";
  import { t } from "@/lib/i18n/index.svelte";
  import { session } from "@/lib/state/session.svelte";
  import type { Group } from "@/types";

  interface Props {
    groups: Group[];
    onSelectGroup: (groupId: string) => void;
    onOpenCreateGroup: () => void;
    onOpenJoinGroup: () => void;
  }

  let { groups, onSelectGroup, onOpenCreateGroup, onOpenJoinGroup }: Props = $props();

  // The groups the user put away are listed apart, folded.
  const active = $derived(groups.filter((g) => !session.isArchived(g.id)));
  const archived = $derived(groups.filter((g) => session.isArchived(g.id)));

  let fileInput = $state<HTMLInputElement>();

  function importChosenFile() {
    const file = fileInput?.files?.[0];
    if (!fileInput || !file) return;
    // So choosing the same file again, once fixed, is a change too.
    fileInput.value = "";
    importGroup(file);
  }
</script>

<input
  bind:this={fileInput}
  type="file"
  accept=".csv,text/csv"
  class="hidden"
  aria-label={t("groups.csvFile")}
  onchange={importChosenFile}
/>

{#snippet card(group: Group)}
  {@const totalCents = group.expenses.reduce((sum, e) => sum + e.amount_cents, 0)}
  {@const members = group.participants.filter((p) => !p.removed).length}
  <li>
    <Card.Root
      class="relative h-full transition-colors has-[button:focus-visible]:ring-3 has-[button:focus-visible]:ring-ring/50 hover:bg-muted/50"
    >
      <Card.Header>
        <Card.Title class="flex items-center gap-2.5">
          {#if group.image}
            <img
              src={group.image}
              alt=""
              class="size-9 shrink-0 rounded-lg object-cover"
              data-testid="group-picture"
            />
          {/if}
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
        {#if group.description}
          <Card.Description class="line-clamp-1">{group.description}</Card.Description>
        {/if}
        <Badge variant="soft" class="w-fit">{group.currency}</Badge>
      </Card.Header>
      <Card.Content class="flex items-end justify-between gap-2">
        <div>
          <div class="text-xs text-muted-foreground">{t("groups.totalSpent")}</div>
          <Amount cents={totalCents} currency={group.currency} class="text-lg font-semibold" />
        </div>
        <div class="flex items-center gap-1 text-xs text-muted-foreground">
          {t("groups.summary", members, group.expenses.length)}
          <ChevronRightIcon class="size-4" aria-hidden="true" />
        </div>
      </Card.Content>
    </Card.Root>
  </li>
{/snippet}

{#snippet cards(list: Group[])}
  <ul class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
    {#each list as group (group.id)}
      {@render card(group)}
    {/each}
  </ul>
{/snippet}

{#if groups.length === 0}
  <Empty.Root class="border border-dashed py-16">
    <Empty.Header>
      <Empty.Media variant="icon"><UsersIcon /></Empty.Media>
      <Empty.Title>{t("groups.empty")}</Empty.Title>
      <Empty.Description>
        {t("groups.emptyHelp")}
      </Empty.Description>
    </Empty.Header>
    <Empty.Content class="flex-row flex-wrap justify-center gap-2">
      <Button onclick={onOpenCreateGroup}>
        <PlusIcon data-icon="inline-start" />
        {t("groups.create")}
      </Button>
      <Button variant="outline" onclick={onOpenJoinGroup}>
        <LinkIcon data-icon="inline-start" />
        {t("groups.joinWithCode")}
      </Button>
      <Button variant="outline" onclick={() => fileInput?.click()}>
        <FileUpIcon data-icon="inline-start" />
        {t("groups.importCsv")}
      </Button>
    </Empty.Content>
  </Empty.Root>
{:else}
  <div class="space-y-4">
    <div class="flex flex-wrap items-end justify-between gap-2">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">{t("groups.yours")}</h1>
        <p class="text-sm text-muted-foreground">
          {t("groups.active", active.length)}
        </p>
      </div>
      <div class="flex flex-wrap gap-2">
        <Button variant="outline" onclick={onOpenJoinGroup}>
          <LinkIcon data-icon="inline-start" />
          {t("groups.joinWithCodeLower")}
        </Button>
        <Button variant="outline" onclick={() => fileInput?.click()}>
          <FileUpIcon data-icon="inline-start" />
          {t("groups.importCsv")}
        </Button>
      </div>
    </div>

    {@render cards(active)}

    {#if archived.length > 0}
      <details class="group/archived">
        <summary
          class="w-fit cursor-pointer rounded-md px-1 py-1 text-sm text-muted-foreground outline-none select-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          {t("archive.section", archived.length)}
        </summary>
        <div class="pt-3">
          {@render cards(archived)}
        </div>
      </details>
    {/if}
  </div>
{/if}
