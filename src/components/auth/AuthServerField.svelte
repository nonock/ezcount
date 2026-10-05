<script lang="ts">
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { t } from "@/lib/i18n/index.svelte";
  import { DEFAULT_SERVER } from "@/lib/server";

  /** The relay the account lives on, for who doesn't use the app's own. */
  let { value = $bindable() }: { value: string } = $props();
</script>

<Field.Field>
  <Field.Label for="input-server">{t("common.server")}</Field.Label>
  <Input
    id="input-server"
    type="url"
    required
    bind:value
    autocomplete="url"
    class="font-mono text-sm"
  />
  <Field.Description>
    {t("auth.serverHelp")}
    {#if value.trim() !== DEFAULT_SERVER}
      <button
        type="button"
        class="underline underline-offset-4 hover:text-foreground"
        onclick={() => (value = DEFAULT_SERVER)}
      >
        {t("auth.defaultServer")}
      </button>
    {/if}
  </Field.Description>
</Field.Field>
