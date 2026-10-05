<script lang="ts">
  import TrashIcon from "@lucide/svelte/icons/trash-2";
  import * as Alert from "@/components/ui/alert";
  import { Button } from "@/components/ui/button";
  import { deleteGroup, refuseDeletion } from "@/lib/actions/groups";
  import { t } from "@/lib/i18n/index.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import type { Group } from "@/types";
  import { formatList } from "@/utils/formatters";

  /**
   * Some members asked to delete the group while its balances aren't settled: who agreed, who
   * hasn't yet, and the user's own answer.
   */
  let { group }: { group: Group } = $props();

  const votes = $derived(group.deletion_votes ?? []);
  const members = $derived(group.participants.filter((p) => !p.removed));
  const names = (agreed: boolean) =>
    formatList(members.filter((p) => votes.includes(p.id) === agreed).map((p) => p.name));
  const iAgreed = $derived(
    openGroup.currentUserId !== null && votes.includes(openGroup.currentUserId)
  );
</script>

{#if votes.length > 0}
  <Alert.Root>
    <TrashIcon />
    <Alert.Title>{t("deletion.asked", names(true), votes.length)}</Alert.Title>
    <Alert.Description>
      <p>{t("deletion.waiting", names(false))}</p>
      <div class="mt-2 flex flex-wrap gap-2">
        {#if iAgreed}
          <Button variant="outline" size="sm" onclick={refuseDeletion}>
            {t("deletion.takeBack")}
          </Button>
        {:else}
          <Button variant="destructive" size="sm" onclick={deleteGroup}>
            {t("deletion.agree")}
          </Button>
          <Button variant="outline" size="sm" onclick={refuseDeletion}>
            {t("deletion.refuse")}
          </Button>
        {/if}
      </div>
    </Alert.Description>
  </Alert.Root>
{/if}
