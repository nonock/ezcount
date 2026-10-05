<script lang="ts">
  import CircleArrowUpIcon from "@lucide/svelte/icons/circle-arrow-up";
  import * as Alert from "@/components/ui/alert";
  import { Button } from "@/components/ui/button";
  import { t } from "@/lib/i18n/index.svelte";

  /**
   * Says this version of the app is too old for something, and what waits for its update
   * (`text`). The web version is always its server's latest: reloading the page updates it.
   */
  let { text }: { text: string } = $props();

  const isWeb = import.meta.env.MODE === "web";
</script>

<Alert.Root data-testid="update-notice">
  <CircleArrowUpIcon />
  <Alert.Title>{t("update.title")}</Alert.Title>
  <Alert.Description>{text}</Alert.Description>
  {#if isWeb}
    <Alert.Action>
      <Button variant="outline" size="sm" onclick={() => location.reload()}>
        {t("update.reload")}
      </Button>
    </Alert.Action>
  {/if}
</Alert.Root>
