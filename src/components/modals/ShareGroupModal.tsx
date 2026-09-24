import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import type { Group, SyncInfo } from "@/types";
import { errorMessage } from "@/utils/errors";
import { formatDateTime } from "@/utils/formatters";
import { CopyIcon, LockIcon, RefreshCwIcon, TriangleAlertIcon } from "lucide-react";
import type React from "react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

const SERVER_KEY = "ezcount_sync_server";
const DEFAULT_SERVER = "http://localhost:8787";

function rememberedServer(): string {
  try {
    return localStorage.getItem(SERVER_KEY) || DEFAULT_SERVER;
  } catch {
    return DEFAULT_SERVER;
  }
}

interface ShareGroupModalProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group;
  syncInfo: SyncInfo | null;
  onEnableSync: (serverUrl: string) => Promise<void>;
  onSyncNow: () => Promise<void>;
}

export const ShareGroupModal: React.FC<ShareGroupModalProps> = ({
  isOpen,
  onClose,
  group,
  syncInfo,
  onEnableSync,
  onSyncNow,
}) => {
  const [serverUrl, setServerUrl] = useState(DEFAULT_SERVER);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setServerUrl(rememberedServer());
    setError(null);
  }, [isOpen]);

  const handleEnable = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await onEnableSync(serverUrl.trim());
      try {
        localStorage.setItem(SERVER_KEY, serverUrl.trim());
      } catch {
        // Remembering the server is only a convenience.
      }
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSyncNow = async () => {
    setBusy(true);
    try {
      await onSyncNow();
    } finally {
      setBusy(false);
    }
  };

  const handleCopy = async () => {
    if (!syncInfo?.invite_code) return;
    try {
      await navigator.clipboard.writeText(syncInfo.invite_code);
      toast.success("Invite code copied");
    } catch {
      document.getElementById("share-invite-code")?.focus();
      toast.info("Select the code and copy it manually");
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Share Group</DialogTitle>
          <DialogDescription>
            {syncInfo?.enabled
              ? `Other members join "${group.name}" with this invite code.`
              : `Sync "${group.name}" so other members can join from their own devices. Everyone can edit, even offline; changes merge when devices reconnect.`}
          </DialogDescription>
        </DialogHeader>

        {!syncInfo?.enabled ? (
          <form onSubmit={handleEnable}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="input-sync-server">Sync server URL</FieldLabel>
                <Input
                  id="input-sync-server"
                  type="url"
                  required
                  value={serverUrl}
                  onChange={(e) => setServerUrl(e.target.value)}
                  placeholder="https://sync.example.com"
                  autoComplete="url"
                />
                <FieldDescription>
                  Changes are end-to-end encrypted: the server cannot read them.
                </FieldDescription>
              </Field>
              <FieldError>{error}</FieldError>
            </FieldGroup>
            <DialogFooter className="mt-6">
              <DialogClose asChild>
                <Button type="button" variant="outline">
                  Cancel
                </Button>
              </DialogClose>
              <Button type="submit" disabled={busy}>
                {busy && <Spinner data-icon="inline-start" />}
                Start Syncing
              </Button>
            </DialogFooter>
          </form>
        ) : (
          <>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="share-invite-code">Invite code</FieldLabel>
                <Textarea
                  id="share-invite-code"
                  readOnly
                  rows={3}
                  value={syncInfo.invite_code ?? ""}
                  onFocus={(e) => e.currentTarget.select()}
                  className="resize-none font-mono text-xs break-all"
                />
                <FieldDescription className="flex items-start gap-1.5">
                  <LockIcon className="mt-0.5 size-3.5 shrink-0" aria-hidden />
                  Anyone with this code can see and edit the group, so share it only with members.
                  Changes are end-to-end encrypted: the sync server cannot read them.
                </FieldDescription>
              </Field>

              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
                <dt className="text-muted-foreground">Server</dt>
                <dd className="truncate text-right font-mono">{syncInfo.server_url}</dd>
                <dt className="text-muted-foreground">Last synced</dt>
                <dd className="text-right">
                  {syncInfo.last_synced_at ? formatDateTime(syncInfo.last_synced_at) : "Never"}
                </dd>
              </dl>

              {syncInfo.last_error && (
                <Alert variant="destructive">
                  <TriangleAlertIcon />
                  <AlertDescription>Last sync failed: {syncInfo.last_error}</AlertDescription>
                </Alert>
              )}
            </FieldGroup>

            <DialogFooter>
              <Button variant="outline" onClick={handleSyncNow} disabled={busy}>
                {busy ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <RefreshCwIcon data-icon="inline-start" />
                )}
                Sync Now
              </Button>
              <Button onClick={handleCopy}>
                <CopyIcon data-icon="inline-start" />
                Copy Invite Code
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
};
