<script lang="ts">
  import Amount from "@/components/common/Amount.svelte";
  import HelpPopover from "@/components/common/HelpPopover.svelte";
  import * as Avatar from "@/components/ui/avatar";
  import { Badge } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import * as Card from "@/components/ui/card";
  import { Progress } from "@/components/ui/progress";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { memberTone } from "@/lib/tones";
  import { cn } from "@/lib/utils";
  import type { Group } from "@/types";

  let { group }: { group: Group } = $props();

  const balances = $derived(openGroup.balances);
  const maxAbs = $derived(Math.max(...balances.map((b) => Math.abs(b.net_cents)), 1));
</script>

<div class="space-y-3">
  <div class="flex items-center gap-1">
    <h2 class="text-base font-semibold">{t("balances.heading")}</h2>
    <HelpPopover title={t("balances.help")}
      >{t("balances.helpPositive")}<span class="font-medium text-positive"
        >{t("balances.green")}</span
      >{t("balances.helpPositiveEnd")}<span class="font-medium text-negative"
        >{t("balances.red")}</span
      >{t("balances.helpNegativeEnd")}</HelpPopover
    >
  </div>

  <ul class="grid grid-cols-1 gap-3 md:grid-cols-2">
    {#each balances as b (b.participant_id)}
      {@const isPositive = b.net_cents > 0}
      {@const isNegative = b.net_cents < 0}
      {@const percentage = Math.min(100, Math.round((Math.abs(b.net_cents) / maxAbs) * 100))}
      <li data-testid="balance-card">
        <Card.Root size="sm" class="h-full">
          <Card.Content class="space-y-3">
            <div class="flex items-center justify-between gap-3">
              <div class="flex min-w-0 items-center gap-2.5">
                <Avatar.Root>
                  <Avatar.Fallback class={memberTone(group, b.participant_id)}
                    >{b.participant_name.charAt(0).toUpperCase()}</Avatar.Fallback
                  >
                </Avatar.Root>
                <div class="min-w-0">
                  <h3 class="flex items-center gap-1.5 truncate font-medium">
                    {b.participant_name}
                    {#if b.removed}
                      <Badge variant="secondary">{t("member.removed")}</Badge>
                    {/if}
                  </h3>
                  <p class="text-xs text-muted-foreground">
                    {isPositive
                      ? t("balances.getsBack")
                      : isNegative
                        ? t("balances.owes")
                        : t("balances.settled")}
                  </p>
                </div>
              </div>
              <Amount
                cents={b.net_cents}
                currency={group.currency}
                tone="balance"
                class="text-base font-semibold"
              />
            </div>

            <Progress
              value={percentage}
              aria-label={t("balances.share", b.participant_name)}
              class={cn(
                "h-1.5",
                isPositive && "**:data-[slot=progress-indicator]:bg-positive",
                isNegative && "**:data-[slot=progress-indicator]:bg-negative"
              )}
            />

            <div class="flex justify-between text-xs text-muted-foreground">
              <span>
                {t("balances.paid")}
                <Amount cents={b.paid_cents} currency={group.currency} class="text-foreground" />
              </span>
              <span>
                {t("balances.consumed")}
                <Amount cents={b.owed_cents} currency={group.currency} class="text-foreground" />
              </span>
            </div>
          </Card.Content>

          {#if isNegative}
            <Card.Footer class="mt-auto">
              <Button
                variant="outline"
                class="w-full"
                onclick={() =>
                  dialogs.openReimburse({
                    fromId: b.participant_id,
                    amount: (Math.abs(b.net_cents) / 100).toFixed(2),
                  })}
              >
                {t("balances.reimburse")}
                <Amount
                  cents={Math.abs(b.net_cents)}
                  currency={group.currency}
                  class="text-muted-foreground"
                />
              </Button>
            </Card.Footer>
          {/if}
        </Card.Root>
      </li>
    {/each}
  </ul>
</div>
