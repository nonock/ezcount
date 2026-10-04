<script lang="ts">
  import { onMount } from "svelte";
  import { renderSVG } from "uqr";
  import { Button } from "@/components/ui/button";
  import * as Field from "@/components/ui/field";
  import { Spinner } from "@/components/ui/spinner";
  import { t } from "@/lib/i18n/index.svelte";
  import { api } from "@/services/api";
  import type { Received } from "@/types";
  import { errorMessage } from "@/utils/errors";

  interface Props {
    /** The relay the phone's account is on. */
    serverUrl: string;
    /** What the phone is to send: its account, or a group's invite. */
    purpose: "login" | "group";
    onReceived: (received: Received) => void;
  }

  /**
   * A QR code for a phone to scan, for a device that can't scan the phone's: once scanned,
   * this device is logged into the phone's account, or joins the group the phone sent. It
   * asks the relay every two seconds, and the code is replaced after two minutes.
   */
  let { serverUrl, purpose, onReceived }: Props = $props();

  const LIFETIME = 120_000;
  const EVERY = 2000;

  let link = $state<string | null>(null);
  let expired = $state(false);
  let error = $state<string | null>(null);
  // Leaving the screen, or a new code, ends the asking for the one before.
  let round = 0;

  // Dark on light whatever the theme: scanners expect it.
  const qrCode = $derived(
    link &&
      `data:image/svg+xml;utf8,${encodeURIComponent(renderSVG(link, { border: 2, blackColor: "#000", whiteColor: "#fff" }))}`
  );

  async function show() {
    const mine = ++round;
    link = null;
    expired = false;
    error = null;
    try {
      const made = await api.receiveLink(serverUrl, purpose);
      if (mine !== round) return;
      link = made;
      const until = Date.now() + LIFETIME;
      while (Date.now() < until) {
        await new Promise((resolve) => setTimeout(resolve, EVERY));
        if (mine !== round) return;
        const received = await api.receive(made);
        if (mine !== round) return;
        if (received) {
          round += 1;
          onReceived(received);
          return;
        }
      }
      expired = true;
    } catch (err) {
      if (mine === round) error = errorMessage(err);
    }
  }

  onMount(() => {
    show();
    return () => {
      round += 1;
    };
  });
</script>

<div class="space-y-3">
  {#if error}
    <Field.Error>{error}</Field.Error>
    <Button variant="outline" onclick={show} class="w-full">{t("link.again")}</Button>
  {:else if expired}
    <p class="text-sm font-medium">{t("link.expired")}</p>
    <Button variant="outline" onclick={show} class="w-full">{t("link.again")}</Button>
  {:else if qrCode}
    <img
      src={qrCode}
      alt={t("receive.qrAlt")}
      width="224"
      height="224"
      class="mx-auto size-56 rounded-lg"
    />
    <p class="flex items-center justify-center gap-2 text-sm text-muted-foreground" role="status">
      <Spinner />
      {t("receive.waiting")}
    </p>
  {:else}
    <div class="flex justify-center py-6"><Spinner /></div>
  {/if}
</div>
