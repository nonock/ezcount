import { LogoMark, Wordmark } from "@/components/common/Logo";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { Group } from "@/types";
import { serverName } from "@/utils/formatters";
import {
  ChevronRightIcon,
  LogOutIcon,
  MonitorIcon,
  MoonIcon,
  PlusIcon,
  SunIcon,
} from "lucide-react";
import { useTheme } from "next-themes";
import type React from "react";

interface NavbarProps {
  currentGroup: Group | null;
  onNavigateHome: () => void;
  onOpenCreateGroup: () => void;
  username: string;
  serverUrl: string;
  onLogOut: () => void;
}

const ThemeMenu: React.FC = () => {
  const { theme = "system", setTheme } = useTheme();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" aria-label="Change theme">
          <SunIcon className="dark:hidden" />
          <MoonIcon className="hidden dark:block" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup value={theme} onValueChange={setTheme}>
          <DropdownMenuRadioItem value="light">
            <SunIcon /> Light
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="dark">
            <MoonIcon /> Dark
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="system">
            <MonitorIcon /> System
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
};

const AccountMenu: React.FC<{ username: string; serverUrl: string; onLogOut: () => void }> = ({
  username,
  serverUrl,
  onLogOut,
}) => (
  <DropdownMenu>
    <DropdownMenuTrigger asChild>
      <Button variant="ghost" size="icon" aria-label="Account">
        <Avatar size="sm" aria-hidden>
          <AvatarFallback>{username.slice(0, 1).toUpperCase()}</AvatarFallback>
        </Avatar>
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" className="min-w-48">
      <DropdownMenuLabel>
        <span className="block text-xs font-normal text-muted-foreground">Logged in as</span>
        <span className="block truncate">{username}</span>
        <span className="block truncate font-mono text-xs font-normal text-muted-foreground">
          {serverName(serverUrl)}
        </span>
      </DropdownMenuLabel>
      <DropdownMenuSeparator />
      <DropdownMenuItem onSelect={onLogOut}>
        <LogOutIcon /> Log out
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
);

export const Navbar: React.FC<NavbarProps> = ({
  currentGroup,
  onNavigateHome,
  onOpenCreateGroup,
  username,
  serverUrl,
  onLogOut,
}) => {
  return (
    <header className="sticky top-0 z-30 border-b bg-background/80 backdrop-blur-lg pt-[env(safe-area-inset-top)]">
      <div className="mx-auto flex h-14 max-w-5xl items-center justify-between gap-3 px-4">
        <nav aria-label="Breadcrumb" className="flex min-w-0 items-center gap-1.5">
          <button
            type="button"
            onClick={onNavigateHome}
            className="flex shrink-0 items-center gap-2 rounded-lg outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
          >
            <LogoMark className="size-8" />
            <Wordmark className="text-base" />
          </button>
          {currentGroup && (
            <>
              <ChevronRightIcon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
              <span className="truncate text-sm text-muted-foreground" aria-current="page">
                {currentGroup.name}
              </span>
            </>
          )}
        </nav>

        <div className="flex shrink-0 items-center gap-1">
          <ThemeMenu />
          <AccountMenu username={username} serverUrl={serverUrl} onLogOut={onLogOut} />
          <Button onClick={onOpenCreateGroup} aria-label="New group">
            <PlusIcon data-icon="inline-start" />
            <span className="hidden sm:inline">New Group</span>
          </Button>
        </div>
      </div>
    </header>
  );
};
