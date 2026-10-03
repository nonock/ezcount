<script lang="ts">
  import { cn } from "@/lib/utils";
  import { moneyParts } from "@/utils/formatters";

  interface Props {
    cents: number;
    currency: string;
    /** `balance` colors by sign (green = gets money back, red = owes) and shows a `+` sign. */
    tone?: "neutral" | "positive" | "balance";
    class?: string;
  }

  let { cents, currency, tone = "neutral", class: className }: Props = $props();

  const color = $derived(
    tone === "positive" || (tone === "balance" && cents > 0)
      ? "text-positive"
      : tone === "balance" && cents < 0
        ? "text-negative"
        : undefined
  );
  const parts = $derived(moneyParts(cents, currency, tone === "balance"));
</script>

<!-- The cents are smaller, so the eye lands on the whole amount first. -->
<span class={cn("whitespace-nowrap tabular-nums", color, className)}
  >{parts.before}<span class="text-[0.8em]">{parts.cents}</span>{parts.after}</span
>
