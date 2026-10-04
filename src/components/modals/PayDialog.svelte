<script lang="ts">
  import CheckIcon from "@lucide/svelte/icons/check";
  import CopyIcon from "@lucide/svelte/icons/copy";
  import { toast } from "svelte-sonner";
  import { renderSVG } from "uqr";
  import Amount from "@/components/common/Amount.svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import { t } from "@/lib/i18n/index.svelte";
  import { formatIban, transferQrText } from "@/lib/sepa";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import type { Group } from "@/types";
  import { amountInput } from "@/utils/formatters";

  /**
   * How to pay someone back by bank transfer: their IBAN, and for euros a QR code that a
   * banking app scans to have the transfer filled in.
   */
  let { group }: { group: Group } = $props();

  const pay = $derived(dialogs.pay);
  const recipient = $derived(group.participants.find((p) => p.id === pay.toId));
  const iban = $derived(recipient?.iban ?? null);
  const reference = $derived(`ezcount ${group.name}`);
  // Dark on light whatever the theme: scanners expect it.
  const qrCode = $derived(
    iban && recipient && group.currency === "EUR" && pay.amountCents > 0
      ? `data:image/svg+xml;utf8,${encodeURIComponent(
          renderSVG(
            transferQrText({
              name: recipient.name,
              iban,
              amountCents: pay.amountCents,
              reference,
            }),
            { ecc: "M", border: 2, blackColor: "#000", whiteColor: "#fff" }
          )
        )}`
      : null
  );

  async function copyIban() {
    if (!iban) return;
    try {
      await navigator.clipboard.writeText(iban);
      toast.success(t("pay.copied"));
    } catch {
      toast.info(t("pay.copyManually"));
    }
  }

  function markPaid() {
    dialogs.pay.open = false;
    dialogs.openReimburse({
      fromId: pay.fromId,
      toId: pay.toId,
      amount: amountInput(pay.amountCents),
    });
  }
</script>

<Dialog.Root bind:open={dialogs.pay.open}>
  <Dialog.Content class="sm:max-w-sm">
    <Dialog.Header>
      <Dialog.Title>{t("pay.title", recipient?.name ?? "")}</Dialog.Title>
      <Dialog.Description>
        {qrCode ? t("pay.scan") : t("pay.transfer")}
      </Dialog.Description>
    </Dialog.Header>

    {#if iban && recipient}
      {#if qrCode}
        <img
          src={qrCode}
          alt={t("pay.qrAlt", recipient.name)}
          width="208"
          height="208"
          class="mx-auto size-52 rounded-lg"
          data-testid="transfer-qr"
        />
      {/if}
      <dl class="grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-1.5 text-sm">
        <dt class="text-muted-foreground">{t("pay.to")}</dt>
        <dd class="truncate text-right">{recipient.name}</dd>
        <dt class="text-muted-foreground">{t("common.amount")}</dt>
        <dd class="text-right">
          <Amount cents={pay.amountCents} currency={group.currency} class="font-semibold" />
        </dd>
        <dt class="text-muted-foreground">{t("pay.iban")}</dt>
        <dd class="flex items-center justify-end gap-1">
          <span class="font-mono text-xs break-all" data-testid="transfer-iban"
            >{formatIban(iban)}</span
          >
          <Button variant="ghost" size="icon-sm" onclick={copyIban} aria-label={t("pay.copy")}>
            <CopyIcon />
          </Button>
        </dd>
        <dt class="text-muted-foreground">{t("pay.reference")}</dt>
        <dd class="truncate text-right">{reference}</dd>
      </dl>
    {/if}

    <Dialog.Footer>
      <Button onclick={markPaid}>
        <CheckIcon data-icon="inline-start" />
        {t("settle.markPaid")}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
