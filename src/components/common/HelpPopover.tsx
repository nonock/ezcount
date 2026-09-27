import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from "@/components/ui/popover";
import { CircleHelpIcon } from "lucide-react";
import type React from "react";

interface HelpPopoverProps {
  title: string;
  children: React.ReactNode;
}

/** A "?" button that explains something in a popover, instead of text always on screen. */
export const HelpPopover: React.FC<HelpPopoverProps> = ({ title, children }) => (
  <Popover>
    <PopoverTrigger asChild>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={`About ${title.toLowerCase()}`}
        className="text-muted-foreground"
      >
        <CircleHelpIcon />
      </Button>
    </PopoverTrigger>
    <PopoverContent align="start" className="w-72">
      <PopoverHeader>
        <PopoverTitle>{title}</PopoverTitle>
        <PopoverDescription>{children}</PopoverDescription>
      </PopoverHeader>
    </PopoverContent>
  </Popover>
);
