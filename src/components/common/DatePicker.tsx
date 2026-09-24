import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { formatDateInput } from "@/utils/formatters";
import { CalendarIcon } from "lucide-react";
import type React from "react";
import { useState } from "react";

interface DatePickerProps {
  id: string;
  /** ID of the visible label, so the button is announced as "<label> <date>". */
  labelId: string;
  /** Local calendar date as `YYYY-MM-DD`. */
  value: string;
  onChange: (value: string) => void;
}

function parseDateInput(value: string): Date | undefined {
  const [year, month, day] = value.split("-").map(Number);
  return year && month && day ? new Date(year, month - 1, day) : undefined;
}

/** First day of the week for the user's locale (0 = Sunday), Monday if unknown. */
function firstDayOfWeek(): 0 | 1 | 2 | 3 | 4 | 5 | 6 {
  try {
    const locale = new Intl.Locale(navigator.language) as Intl.Locale & {
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

export const DatePicker: React.FC<DatePickerProps> = ({ id, labelId, value, onChange }) => {
  const [open, setOpen] = useState(false);
  const date = parseDateInput(value);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          type="button"
          variant="outline"
          aria-labelledby={`${labelId} ${id}`}
          className="w-full justify-start font-normal dark:bg-input/30"
        >
          <CalendarIcon data-icon="inline-start" className="text-muted-foreground" />
          {date ? date.toLocaleDateString(undefined, { dateStyle: "medium" }) : "Pick a date"}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align="start" collisionPadding={12}>
        <Calendar
          mode="single"
          selected={date}
          defaultMonth={date}
          weekStartsOn={firstDayOfWeek()}
          // Bigger day cells on phones so they're easy to tap.
          className="[--cell-size:--spacing(9)] sm:[--cell-size:--spacing(7)]"
          onSelect={(picked) => {
            if (!picked) return;
            onChange(formatDateInput(picked));
            setOpen(false);
          }}
          autoFocus
        />
      </PopoverContent>
    </Popover>
  );
};
