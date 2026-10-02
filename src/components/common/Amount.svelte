<script lang="ts">
  import { cn } from "@/lib/utils";
  import { formatMoney } from "@/utils/formatters";

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
</script>

<span class={cn("tabular-nums", color, className)}
  >{tone === "balance" && cents > 0 ? "+" : ""}{formatMoney(cents, currency)}</span
>
