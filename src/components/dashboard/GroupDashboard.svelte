<script lang="ts">
  import FileUpIcon from "@lucide/svelte/icons/file-up";
  import LinkIcon from "@lucide/svelte/icons/link";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import UsersIcon from "@lucide/svelte/icons/users";
  import Amount from "@/components/common/Amount.svelte";
  import { Button } from "@/components/ui/button";
  import * as Empty from "@/components/ui/empty";
  import { importGroup } from "@/lib/actions/groups";
  import { groupNets, totalsByCurrency } from "@/lib/dashboard";
  import { t } from "@/lib/i18n/index.svelte";
  import { session } from "@/lib/state/session.svelte";
  import type { Group } from "@/types";
  import GroupCard from "./GroupCard.svelte";

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

  const nets = $derived(groupNets(groups, (groupId) => session.identityIn(groupId)));
  const totals = $derived(totalsByCurrency(active, nets));

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

{#snippet cards(list: Group[])}
  <ul class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
    {#each list as group (group.id)}
      <GroupCard {group} net={nets.get(group.id) ?? 0} onSelect={onSelectGroup} />
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

    {#if totals.length > 0}
      <section
        aria-label={t("groups.overall")}
        class="grid grid-cols-2 gap-x-6 gap-y-1 rounded-xl bg-card px-4 py-3 ring-1 ring-foreground/10 sm:w-fit"
      >
        <h2 class="text-xs text-muted-foreground">{t("groups.owedToYou")}</h2>
        <h2 class="text-xs text-muted-foreground">{t("groups.youOweTotal")}</h2>
        {#each totals as total (total.currency)}
          <Amount
            cents={total.owed}
            currency={total.currency}
            class={total.owed > 0 ? "text-lg font-semibold text-positive" : "text-lg font-semibold"}
          />
          <Amount
            cents={total.owes}
            currency={total.currency}
            class={total.owes > 0 ? "text-lg font-semibold text-negative" : "text-lg font-semibold"}
          />
        {/each}
      </section>
    {/if}

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
