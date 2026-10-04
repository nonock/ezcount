<script lang="ts" module>
  /** One line of an expense as it is typed. */
  export interface ItemState {
    key: number;
    name: string;
    // A number once typed in: Svelte binds number inputs as numbers.
    amount: string | number | null;
    people: string[];
  }

  let nextKey = 0;

  export function newItem(people: string[], name = "", amount: ItemState["amount"] = "") {
    nextKey += 1;
    return { key: nextKey, name, amount, people } satisfies ItemState;
  }
</script>

<script lang="ts">
  import PlusIcon from "@lucide/svelte/icons/plus";
  import XIcon from "@lucide/svelte/icons/x";
  import { badgeVariants } from "@/components/ui/badge";
  import { Button } from "@/components/ui/button";
  import { Input } from "@/components/ui/input";
  import { t } from "@/lib/i18n/index.svelte";
  import { cn } from "@/lib/utils";

  interface Props {
    items: ItemState[];
    participants: { id: string; name: string; removed?: boolean }[];
    /** After an amount changed or a line went: the expense's amount follows. */
    onChange: () => void;
  }

  /** An expense line by line, as on a receipt: what each line cost, and who it was for. */
  let { items = $bindable(), participants, onChange }: Props = $props();

  // A new line is for everyone still in the group; who it wasn't for is taken off.
  const everyone = $derived(participants.filter((p) => !p.removed).map((p) => p.id));

  function add() {
    items.push(newItem([...everyone]));
  }

  function remove(index: number) {
    items.splice(index, 1);
    onChange();
  }

  function toggle(item: ItemState, id: string) {
    item.people = item.people.includes(id)
      ? item.people.filter((p) => p !== id)
      : // In the group's order, which decides who gets an odd cent.
        participants.map((p) => p.id).filter((p) => p === id || item.people.includes(p));
  }
</script>

<ul class="space-y-2" aria-label={t("expense.items")}>
  {#each items as item, i (item.key)}
    <li class="space-y-2 rounded-xl border p-2.5" aria-label={t("expense.itemLabel", i + 1)}>
      <div class="flex items-center gap-2">
        <Input
          bind:value={item.name}
          maxlength={80}
          placeholder={t("expense.itemName")}
          aria-label={t("expense.itemNameLabel", i + 1)}
          class="h-8 min-w-0 flex-1"
        />
        <Input
          type="number"
          inputmode="decimal"
          step="0.01"
          min="0.01"
          bind:value={item.amount}
          oninput={onChange}
          placeholder="0.00"
          aria-label={t("expense.itemAmountLabel", i + 1)}
          class="h-8 w-24 text-right tabular-nums"
        />
        <Button
          variant="ghost"
          size="icon-sm"
          onclick={() => remove(i)}
          aria-label={t("expense.removeItem", i + 1)}
        >
          <XIcon />
        </Button>
      </div>
      <fieldset class="flex min-w-0 flex-wrap gap-1" aria-label={t("expense.itemFor", i + 1)}>
        {#each participants as p (p.id)}
          {@const on = item.people.includes(p.id)}
          <button
            type="button"
            aria-pressed={on}
            onclick={() => toggle(item, p.id)}
            class={cn(
              badgeVariants({ variant: on ? "default" : "outline" }),
              "h-6 cursor-pointer",
              !on && "text-muted-foreground"
            )}
          >
            {p.name}
          </button>
        {/each}
      </fieldset>
    </li>
  {/each}
</ul>
<Button variant="outline" size="sm" onclick={add} class="w-fit">
  <PlusIcon data-icon="inline-start" />
  {t("expense.addItem")}
</Button>
