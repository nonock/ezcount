<script lang="ts">
  import CircleHelpIcon from "@lucide/svelte/icons/circle-question-mark";
  import type { Snippet } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Popover from "@/components/ui/popover";
  import { t } from "@/lib/i18n/index.svelte";

  /** A "?" button that explains something in a popover, instead of text always on screen. */
  let { title, children }: { title: string; children: Snippet } = $props();
</script>

<Popover.Root>
  <Popover.Trigger>
    {#snippet child({ props })}
      <Button
        {...props}
        variant="ghost"
        size="icon-sm"
        aria-label={t("common.about", title)}
        class="text-muted-foreground"
      >
        <CircleHelpIcon />
      </Button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content align="start" class="w-72">
    <Popover.Header>
      <Popover.Title>{title}</Popover.Title>
      <Popover.Description>{@render children()}</Popover.Description>
    </Popover.Header>
  </Popover.Content>
</Popover.Root>
