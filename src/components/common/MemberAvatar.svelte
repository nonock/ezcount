<script lang="ts">
  import * as Avatar from "@/components/ui/avatar";
  import { memberTone } from "@/lib/tones";
  import type { Group } from "@/types";

  interface Props {
    group: Group;
    participantId: string;
    /** For someone the group no longer lists. */
    name?: string;
    size?: "default" | "sm" | "lg";
  }

  /** A member's picture, or their initial on their color. Their name is always next to it. */
  let { group, participantId, name, size = "default" }: Props = $props();

  const member = $derived(group.participants.find((p) => p.id === participantId));
</script>

<Avatar.Root {size} aria-hidden="true">
  {#if member?.avatar}
    <img src={member.avatar} alt="" class="size-full rounded-full object-cover" />
  {:else}
    <Avatar.Fallback class={memberTone(group, participantId)}>
      {(name ?? member?.name ?? "").charAt(0).toUpperCase()}
    </Avatar.Fallback>
  {/if}
</Avatar.Root>
