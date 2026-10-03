<script lang="ts">
  import { getLocalTimeZone, parseDate } from "@internationalized/date";
  import CalendarIcon from "@lucide/svelte/icons/calendar";
  import { Button } from "@/components/ui/button";
  import { Calendar } from "@/components/ui/calendar";
  import * as Popover from "@/components/ui/popover";
  import { i18n, t } from "@/lib/i18n/index.svelte";

  interface Props {
    id: string;
    /** ID of the visible label, so the button is announced as "<label> <date>". */
    labelId: string;
    /** Local calendar date as `YYYY-MM-DD`. */
    value: string;
    onChange: (value: string) => void;
  }

  let { id, labelId, value, onChange }: Props = $props();
  let open = $state(false);

  const date = $derived.by(() => {
    try {
      return value ? parseDate(value) : undefined;
    } catch {
      return undefined;
    }
  });

  /** First day of the week for the user's locale (0 = Sunday), Monday if unknown. */
  function firstDayOfWeek(): 0 | 1 | 2 | 3 | 4 | 5 | 6 {
    try {
      const locale = new Intl.Locale(i18n.locale) as Intl.Locale & {
        getWeekInfo?: () => { firstDay: number };
        weekInfo?: { firstDay: number };
      };
      const firstDay = (locale.getWeekInfo?.() ?? locale.weekInfo)?.firstDay;
      if (firstDay) return (firstDay % 7) as 0 | 1 | 2 | 3 | 4 | 5 | 6;
    } catch {
      // Older engines: fall through to the default.
    }
    return 1;
  }
</script>

<Popover.Root bind:open>
  <Popover.Trigger>
    {#snippet child({ props })}
      <Button
        {...props}
        {id}
        variant="ghost"
        aria-labelledby={`${labelId} ${id}`}
        class="w-full justify-start border-input bg-card px-3 font-normal hover:bg-card has-data-[icon=inline-start]:pl-3 aria-expanded:bg-card"
      >
        <CalendarIcon data-icon="inline-start" class="text-muted-foreground" />
        {date
          ? date.toDate(getLocalTimeZone()).toLocaleDateString(i18n.locale, { dateStyle: "medium" })
          : t("date.pick")}
      </Button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content class="w-auto p-0" align="start" collisionPadding={12}>
    <Calendar
      type="single"
      value={date}
      placeholder={date}
      weekStartsOn={firstDayOfWeek()}
      locale={i18n.locale}
      class="[--cell-size:--spacing(9)] sm:[--cell-size:--spacing(7)]"
      onValueChange={(picked) => {
        if (!picked) return;
        onChange(picked.toString());
        open = false;
      }}
      initialFocus
    />
  </Popover.Content>
</Popover.Root>
