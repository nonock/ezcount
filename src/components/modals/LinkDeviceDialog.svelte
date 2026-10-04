<script lang="ts">
  import ScanQrCodeIcon from "@lucide/svelte/icons/scan-qr-code";
  import ShieldAlertIcon from "@lucide/svelte/icons/shield-alert";
  import { untrack } from "svelte";
  import { renderSVG } from "uqr";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import { sendLoginToScanned } from "@/lib/actions";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { api } from "@/services/api";
  import { nativeFeatures } from "@/services/native.svelte";
  import { errorMessage } from "@/utils/errors";

  /**
   * Logs another device into the account: asks for the password, then shows a QR code to
   * scan from that device's login screen. The code works once and for a short time.
   */

  let password = $state("");
  let link = $state<string | null>(null);
  // When the code stops working, and the seconds left until then.
  let expiresAt = 0;
  let secondsLeft = $state(0);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  $effect.pre(() => {
    if (!dialogs.linkDevice) return;
    untrack(() => {
      password = "";
      link = null;
      error = null;
    });
  });

  // Dark on light whatever the theme: scanners expect it.
  const qrCode = $derived(
    link &&
      `data:image/svg+xml;utf8,${encodeURIComponent(renderSVG(link, { border: 2, blackColor: "#000", whiteColor: "#fff" }))}`
  );
  const expired = $derived(link !== null && secondsLeft <= 0);
  const timeLeft = $derived(
    `${Math.floor(secondsLeft / 60)}:${String(secondsLeft % 60).padStart(2, "0")}`
  );

  $effect(() => {
    if (!link || !dialogs.linkDevice) return;
    const tick = () => {
      secondsLeft = Math.max(0, Math.ceil((expiresAt - Date.now()) / 1000));
    };
    const timer = setInterval(tick, 500);
    return () => clearInterval(timer);
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    submitting = true;
    error = null;
    try {
      const created = await api.createLoginLink(password);
      password = "";
      expiresAt = Date.now() + created.expires_in * 1000;
      secondsLeft = created.expires_in;
      link = created.link;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog.Root bind:open={dialogs.linkDevice}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("link.title")}</Dialog.Title>
      <Dialog.Description>
        {link && !expired ? t("link.help") : t("link.intro")}
      </Dialog.Description>
    </Dialog.Header>

    {#if qrCode && !expired}
      <div class="space-y-3">
        <img
          src={qrCode}
          alt={t("link.qrAlt")}
          width="224"
          height="224"
          class="mx-auto size-56 rounded-lg"
        />
        <p class="text-center text-sm" aria-live="off">
          {t("link.expiresIn", timeLeft)}
        </p>
        <p class="flex items-start gap-1.5 text-sm text-muted-foreground">
          <ShieldAlertIcon class="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
          {t("link.warning")}
        </p>
      </div>
      <Dialog.Footer>
        <Button onclick={() => (dialogs.linkDevice = false)}>{t("common.done")}</Button>
      </Dialog.Footer>
    {:else}
      <form onsubmit={handleSubmit}>
        <Field.Group>
          {#if expired}
            <p class="text-sm font-medium">{t("link.expired")}</p>
          {/if}
          <Field.Field>
            <Field.Label for="link-device-password">{t("common.password")}</Field.Label>
            <Input
              id="link-device-password"
              type="password"
              required
              bind:value={password}
              autocomplete="current-password"
            />
          </Field.Field>
          {#if error}
            <Field.Error>{error}</Field.Error>
          {/if}
        </Field.Group>
        <Dialog.Footer class="mt-6">
          <Dialog.Close>
            {#snippet child({ props })}
              <Button {...props} variant="outline">{t("common.cancel")}</Button>
            {/snippet}
          </Dialog.Close>
          <Button type="submit" disabled={submitting}>
            {#if submitting}
              <Spinner data-icon="inline-start" />
            {/if}
            {expired ? t("link.again") : t("link.show")}
          </Button>
          {#if nativeFeatures.scan}
            <!-- For a device that can't scan: it shows a code on its login screen. -->
            <Button
              variant="outline"
              disabled={submitting || !password}
              onclick={() => sendLoginToScanned(password)}
            >
              <ScanQrCodeIcon data-icon="inline-start" />
              {t("link.scan")}
            </Button>
          {/if}
        </Dialog.Footer>
      </form>
    {/if}
  </Dialog.Content>
</Dialog.Root>
