// Phone and OS integrations: share sheet, QR scanning and invite links opening the app.
import {
  Format,
  cancel,
  checkPermissions,
  requestPermissions,
  scan,
} from "@tauri-apps/plugin-barcode-scanner";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { useEffect, useState } from "react";
import type { NativeFeatures } from "../types";
import { api } from "./api";

const NONE: NativeFeatures = { share: false, scan: false };
let features: Promise<NativeFeatures> | null = null;

/** What this device offers beyond the web view. Nothing until known. */
export function useNativeFeatures(): NativeFeatures {
  const [value, setValue] = useState(NONE);
  useEffect(() => {
    features ??= api.nativeFeatures().catch(() => NONE);
    let active = true;
    features.then((f) => active && setValue(f));
    return () => {
      active = false;
    };
  }, []);
  return value;
}

/** Invite links (`…/join#…`) and the `ezcount://join?…` links the join page opens. */
export function isInviteLink(url: string): boolean {
  return url.startsWith("ezcount://join") || /^https?:\/\/[^#]*\/join\/?#/.test(url);
}

/**
 * Calls `handler` with each invite link the app is opened with: the one that launched it,
 * then any opened while it runs. Returns the unsubscribe function.
 */
export function onInviteLink(handler: (link: string) => void): () => void {
  const handle = (urls: string[] | null) => {
    const link = urls?.find(isInviteLink);
    if (link) handler(link);
  };
  getCurrent()
    .then(handle)
    .catch((err) => console.error("Could not read the link the app was opened with:", err));
  const unlisten = onOpenUrl(handle).catch((err) => {
    console.error("Could not listen for links:", err);
    return () => {};
  });
  return () => {
    unlisten.then((f) => f());
  };
}

export class ScanCancelled extends Error {}

/**
 * Scans a QR code with the camera and returns its text. The camera shows *behind* the web
 * view, so the caller hides the app while this runs (see `.scanning` in styles.css).
 */
export async function scanQrCode(): Promise<string> {
  let permission = await checkPermissions();
  if (permission !== "granted") permission = await requestPermissions();
  if (permission !== "granted") {
    throw new Error("ezcount needs the camera to scan. Allow it in the phone's settings.");
  }
  try {
    return (await scan({ windowed: true, formats: [Format.QRCode] })).content;
  } catch (err) {
    if (cancelling) throw new ScanCancelled();
    throw err;
  } finally {
    cancelling = false;
  }
}

let cancelling = false;

export async function cancelScan(): Promise<void> {
  cancelling = true;
  await cancel();
}
