<script lang="ts">
  import { cn } from "@/lib/utils";
  import type { PasswordStrength } from "@/types";

  /**
   * A four-part bar and a short verdict for a new password, with what to change while it is
   * too weak. `id` goes on the verdict, for the password field's `aria-describedby`.
   */
  let { strength, id }: { strength: PasswordStrength; id: string } = $props();

  // By zxcvbn score, 0 to 4. Sign-up needs 3.
  const LEVELS = [
    { label: "Too weak", color: "bg-negative" },
    { label: "Too weak", color: "bg-negative" },
    { label: "Weak", color: "bg-warning" },
    { label: "Good", color: "bg-positive" },
    { label: "Strong", color: "bg-positive" },
  ];

  const level = $derived(LEVELS[strength.score] ?? LEVELS[0]);
  // Strong enough but under the minimum length: only length is missing.
  const label = $derived(!strength.acceptable && strength.score >= 3 ? "Too short" : level.label);
  const color = $derived(
    strength.acceptable ? level.color : strength.score >= 3 ? "bg-warning" : level.color
  );
  const hint = $derived(
    strength.acceptable ? null : (strength.warning ?? strength.suggestions[0] ?? null)
  );
</script>

<div class="space-y-1.5">
  <div class="grid grid-cols-4 gap-1" aria-hidden="true">
    {#each [1, 2, 3, 4] as segment (segment)}
      <div
        class={cn(
          "h-1.5 rounded-full transition-colors",
          segment <= Math.max(1, strength.score) ? color : "bg-muted"
        )}
      ></div>
    {/each}
  </div>
  <p {id} aria-live="polite" class="text-sm text-muted-foreground">
    <span class="font-medium text-foreground">{label}</span>
    {#if hint}
      {` · ${hint}`}
    {/if}
  </p>
</div>
