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
  KeyRoundIcon,
  LockKeyholeIcon,
  LogOutIcon,
  MenuIcon,
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
  onChangePassword: () => void;
  onNewRecoveryKey: () => void;
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

type AccountMenuProps = Pick<
  NavbarProps,
  "username" | "serverUrl" | "onChangePassword" | "onNewRecoveryKey" | "onLogOut"
>;

const AccountMenu: React.FC<AccountMenuProps> = ({
  username,
  serverUrl,
  onChangePassword,
  onNewRecoveryKey,
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
      <DropdownMenuItem onSelect={onChangePassword}>
        <LockKeyholeIcon /> Change password
      </DropdownMenuItem>
      <DropdownMenuItem onSelect={onNewRecoveryKey}>
        <KeyRoundIcon /> New recovery key
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem onSelect={onLogOut}>
        <LogOutIcon /> Log out
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
);

/** On phones, everything in one menu: the account, the theme and the account's settings. */
const PhoneMenu: React.FC<AccountMenuProps> = ({
  username,
  serverUrl,
  onChangePassword,
  onNewRecoveryKey,
  onLogOut,
}) => {
  const { theme = "system", setTheme } = useTheme();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" aria-label="Menu">
          <MenuIcon />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-56">
        <DropdownMenuLabel>
          <span className="block text-xs font-normal text-muted-foreground">Logged in as</span>
          <span className="block truncate">{username}</span>
          <span className="block truncate font-mono text-xs font-normal text-muted-foreground">
            {serverName(serverUrl)}
          </span>
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">
          Theme
        </DropdownMenuLabel>
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
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={onChangePassword}>
          <LockKeyholeIcon /> Change password
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={onNewRecoveryKey}>
          <KeyRoundIcon /> New recovery key
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={onLogOut}>
          <LogOutIcon /> Log out
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
};

export const Navbar: React.FC<NavbarProps> = ({
  currentGroup,
  onNavigateHome,
  onOpenCreateGroup,
  username,
  serverUrl,
  onChangePassword,
  onNewRecoveryKey,
  onLogOut,
}) => {
  return (
    <header className="sticky top-0 z-30 border-b bg-background/80 backdrop-blur-lg pt-[env(safe-area-inset-top)]">
      <div className="mx-auto flex h-14 max-w-5xl items-center justify-between gap-3 px-4">
        {/* On phones the logo is in the bottom bar, and the header names the app. */}
        <Wordmark className="text-lg sm:hidden" />
        <nav aria-label="Breadcrumb" className="flex min-w-0 items-center gap-1.5 max-sm:hidden">
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

        <div className="sm:hidden">
          <PhoneMenu
            username={username}
            serverUrl={serverUrl}
            onChangePassword={onChangePassword}
            onNewRecoveryKey={onNewRecoveryKey}
            onLogOut={onLogOut}
          />
        </div>
        <div className="flex shrink-0 items-center gap-1 max-sm:hidden">
          <ThemeMenu />
          <AccountMenu
            username={username}
            serverUrl={serverUrl}
            onChangePassword={onChangePassword}
            onNewRecoveryKey={onNewRecoveryKey}
            onLogOut={onLogOut}
          />
          <Button onClick={onOpenCreateGroup} aria-label="New group">
            <PlusIcon data-icon="inline-start" />
            <span className="hidden sm:inline">New Group</span>
          </Button>
        </div>
      </div>
    </header>
  );
};
