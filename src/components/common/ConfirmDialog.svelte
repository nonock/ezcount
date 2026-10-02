<script lang="ts">
  import * as AlertDialog from "@/components/ui/alert-dialog";
  import { confirmation } from "@/lib/state/confirm.svelte";

  // The app's one confirmation dialog: see `askConfirm`.
  const options = $derived(confirmation.options);
</script>

<AlertDialog.Root
  bind:open={
    () => confirmation.open,
    (open) => {
      if (!open) confirmation.settle(false);
    }
  }
>
  <AlertDialog.Content size="sm">
    <AlertDialog.Header>
      <AlertDialog.Title>{options.title}</AlertDialog.Title>
      {#if options.description}
        <AlertDialog.Description>{options.description}</AlertDialog.Description>
      {/if}
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>{options.cancelLabel ?? "Cancel"}</AlertDialog.Cancel>
      <AlertDialog.Action
        variant={options.destructive ? "destructive" : "default"}
        onclick={() => confirmation.settle(true)}
      >
        {options.confirmLabel ?? "Confirm"}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
