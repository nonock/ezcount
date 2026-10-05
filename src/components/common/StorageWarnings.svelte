<script lang="ts">
  import TriangleAlertIcon from "@lucide/svelte/icons/triangle-alert";
  import * as Alert from "@/components/ui/alert";
  import { Button } from "@/components/ui/button";
  import { backendText } from "@/lib/i18n/backend";
  import { t } from "@/lib/i18n/index.svelte";

  /**
   * What the app couldn't read of its saved data when it started. The data is left where it
   * is; dismissing the warnings empties the list.
   */
  let { warnings = $bindable() }: { warnings: string[] } = $props();
</script>

{#if warnings.length > 0}
  <Alert.Root>
    <TriangleAlertIcon />
    <Alert.Title>{t("app.storageWarnings")}</Alert.Title>
    <Alert.Description>
      <ul class="list-disc pl-4">
        {#each warnings as warning (warning)}
          <li>{backendText(warning)}</li>
        {/each}
      </ul>
      <p>{t("app.storageKept")}</p>
    </Alert.Description>
    <Alert.Action>
      <Button variant="ghost" size="sm" onclick={() => (warnings = [])}>
        {t("common.dismiss")}
      </Button>
    </Alert.Action>
  </Alert.Root>
{/if}
