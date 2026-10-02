<script lang="ts">
  import CheckIcon from "@lucide/svelte/icons/check";
  import UserPlusIcon from "@lucide/svelte/icons/user-plus";
  import { untrack } from "svelte";
  import * as Avatar from "@/components/ui/avatar";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import { addSelf, chooseIdentity, skipIdentity } from "@/lib/actions";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { session } from "@/lib/state/session.svelte";
  import type { Group } from "@/types";
  import { errorMessage } from "@/utils/errors";

  /** Asks which participant the user is. The answer is saved in their account. */
  let { group }: { group: Group } = $props();

  let addingSelf = $state(false);
  let name = $state("");
  let busyId = $state<string | null>(null);
  let error = $state<string | null>(null);

  // Typing the name is all that's left once the user says they're not in the list.
  let selfInput = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (addingSelf) selfInput?.focus();
  });

  const currentUserId = $derived(openGroup.currentUserId);
  const people = $derived(group.participants.filter((p) => !p.removed));

  $effect.pre(() => {
    if (!dialogs.who) return;
    const suggested = session.account?.username ?? "";
    untrack(() => {
      addingSelf = false;
      name = suggested;
      busyId = null;
      error = null;
    });
  });

  async function run(id: string, action: () => Promise<void>) {
    busyId = id;
    error = null;
    try {
      await action();
      dialogs.who = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      busyId = null;
    }
  }

  function handleAddSelf(e: SubmitEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (trimmed) run("new", () => addSelf(trimmed));
  }
</script>

<Dialog.Root
  bind:open={
    () => dialogs.who,
    (open) => {
      if (!open) skipIdentity();
    }
  }
>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-sm">
    <Dialog.Header>
      <Dialog.Title>Who are you in "{group.name}"?</Dialog.Title>
      <Dialog.Description>
        Your balance and "Paid by you" follow this choice on all your devices.
      </Dialog.Description>
    </Dialog.Header>

    {#if !addingSelf}
      <ul class="space-y-1.5" aria-label="Members">
        {#each people as p (p.id)}
          <li>
            <Button
              variant={p.id === currentUserId ? "secondary" : "outline"}
              class="h-11 w-full justify-start gap-3"
              disabled={busyId !== null}
              onclick={() => run(p.id, () => chooseIdentity(p.id))}
            >
              <Avatar.Root size="sm" aria-hidden="true">
                <Avatar.Fallback>{p.name.slice(0, 1).toUpperCase()}</Avatar.Fallback>
              </Avatar.Root>
              <span class="truncate">{p.name}</span>
              {#if busyId === p.id}
                <Spinner class="ml-auto" />
              {:else if p.id === currentUserId}
                <CheckIcon class="ml-auto" aria-label="You" />
              {/if}
            </Button>
          </li>
        {/each}
      </ul>
      {#if error}
        <Field.Error>{error}</Field.Error>
      {/if}
      <Dialog.Footer>
        <Button variant="ghost" onclick={() => (addingSelf = true)}>
          <UserPlusIcon data-icon="inline-start" />
          I'm not in the list
        </Button>
      </Dialog.Footer>
    {:else}
      <form onsubmit={handleAddSelf}>
        <Field.Group>
          <Field.Field>
            <Field.Label for="input-self-name">Your name in this group</Field.Label>
            <Input id="input-self-name" required bind:ref={selfInput} bind:value={name} />
          </Field.Field>
          {#if error}
            <Field.Error>{error}</Field.Error>
          {/if}
        </Field.Group>
        <Dialog.Footer class="mt-6">
          <Button variant="outline" onclick={() => (addingSelf = false)}>Back</Button>
          <Button type="submit" disabled={busyId !== null}>
            {#if busyId === "new"}
              <Spinner data-icon="inline-start" />
            {/if}
            Add Me
          </Button>
        </Dialog.Footer>
      </form>
    {/if}
  </Dialog.Content>
</Dialog.Root>
