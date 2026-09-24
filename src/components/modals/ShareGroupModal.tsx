import type React from "react";
import { useEffect, useState } from "react";
import type { Group, SyncInfo } from "../../types";
import { formatDateTime } from "../../utils/formatters";
import { Modal } from "../common/Modal";

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
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    setServerUrl(rememberedServer());
    setError(null);
    setCopied(false);
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
      setError(err instanceof Error ? err.message : String(err));
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
      setCopied(true);
    } catch {
      document.getElementById("share-invite-code")?.focus();
    }
  };

  const inputClass =
    "w-full px-3.5 py-2.5 rounded-xl bg-slate-950 border border-slate-800 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition";

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Share Group" icon="🔗">
      {!syncInfo?.enabled ? (
        <form onSubmit={handleEnable} className="space-y-4">
          <p className="text-xs text-slate-400 leading-relaxed">
            Sync <strong className="text-slate-200">{group.name}</strong> through a sync server so
            other members can join from their own devices. Everyone can add and edit expenses, even
            offline; changes merge automatically when devices reconnect.
          </p>
          <div>
            <label
              htmlFor="input-sync-server"
              className="block text-xs font-semibold text-slate-300 mb-1.5"
            >
              Sync server URL *
            </label>
            <input
              id="input-sync-server"
              type="url"
              required
              value={serverUrl}
              onChange={(e) => setServerUrl(e.target.value)}
              placeholder="https://sync.example.com"
              autoComplete="url"
              className={inputClass}
            />
          </div>
          {error && (
            <p role="alert" className="text-xs text-rose-300">
              {error}
            </p>
          )}
          <div className="flex items-center justify-end gap-2.5 pt-3 border-t border-slate-800">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 rounded-xl text-xs font-semibold text-slate-400 hover:text-slate-200 transition cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={busy}
              className="px-5 py-2 rounded-xl text-xs font-semibold bg-indigo-600 hover:bg-indigo-500 text-white shadow-md shadow-indigo-600/20 transition cursor-pointer disabled:opacity-50"
            >
              {busy ? "Connecting…" : "Start Syncing"}
            </button>
          </div>
        </form>
      ) : (
        <div className="space-y-4">
          <div>
            <label
              htmlFor="share-invite-code"
              className="block text-xs font-semibold text-slate-300 mb-1.5"
            >
              Invite code
            </label>
            <textarea
              id="share-invite-code"
              readOnly
              rows={3}
              value={syncInfo.invite_code ?? ""}
              onFocus={(e) => e.currentTarget.select()}
              className={`${inputClass} font-mono text-[11px] break-all resize-none`}
            />
            <p className="text-[11px] text-slate-500 mt-1.5">
              Anyone with this code can see and edit the group, so share it only with members.
              Changes are end-to-end encrypted: the sync server cannot read them.
            </p>
          </div>

          <dl className="text-xs space-y-1">
            <div className="flex justify-between gap-3">
              <dt className="text-slate-400">Server</dt>
              <dd className="text-slate-200 font-mono truncate">{syncInfo.server_url}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-slate-400">Last synced</dt>
              <dd className="text-slate-200">
                {syncInfo.last_synced_at ? formatDateTime(syncInfo.last_synced_at) : "Never"}
              </dd>
            </div>
          </dl>
          {syncInfo.last_error && (
            <p role="alert" className="text-xs text-rose-300">
              Last sync failed: {syncInfo.last_error}
            </p>
          )}

          <div className="flex items-center justify-end gap-2.5 pt-3 border-t border-slate-800">
            <button
              type="button"
              onClick={handleSyncNow}
              disabled={busy}
              className="px-4 py-2 rounded-xl text-xs font-semibold text-slate-300 hover:text-white bg-slate-800 hover:bg-slate-700 border border-slate-700 transition cursor-pointer disabled:opacity-50"
            >
              {busy ? "Syncing…" : "Sync Now"}
            </button>
            <button
              type="button"
              onClick={handleCopy}
              className="px-5 py-2 rounded-xl text-xs font-semibold bg-indigo-600 hover:bg-indigo-500 text-white shadow-md shadow-indigo-600/20 transition cursor-pointer"
            >
              {copied ? "Copied" : "Copy Invite Code"}
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
};
