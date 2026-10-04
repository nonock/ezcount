<script lang="ts">
  import * as Dialog from "@/components/ui/dialog";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import type { Group } from "@/types";
  import ReimbursementForm from "./ReimbursementForm.svelte";

  let { group }: { group: Group } = $props();
</script>

<Dialog.Root bind:open={dialogs.reimburse.open}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("reimburse.title")}</Dialog.Title>
      <Dialog.Description>{t("reimburse.intro")}</Dialog.Description>
    </Dialog.Header>

    <!-- The content is made anew at each opening, and the form with it. -->
    <ReimbursementForm
      {group}
      prefill={dialogs.reimburse}
      onDone={() => (dialogs.reimburse.open = false)}
    />
  </Dialog.Content>
</Dialog.Root>
