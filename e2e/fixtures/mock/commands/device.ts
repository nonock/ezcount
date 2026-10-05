// What the device does for the app: its features, saving and sharing files, the camera, the
// link the app was opened with, and the feedback form. What the app saved, shared, sent or
// opened in the browser lands on `window` for the tests to read.

import { clone, requireAccount, w } from "../state";
import type { Commands } from "../types";

export const deviceCommands: Commands = {
  native_features: () => ({ share: false, scan: false, save: false, ...w.__NATIVE__ }),

  "plugin:app|version": () => "0.0.0-test",

  "plugin:deep-link|get_current": () => (w.__OPENED_WITH__ ? [w.__OPENED_WITH__] : null),

  "plugin:barcode-scanner|check_permissions": () => ({ camera: "granted" }),

  "plugin:barcode-scanner|scan": () => ({
    content: w.__SCANNED__,
    format: "QR_CODE",
    bounds: null,
  }),

  "plugin:opener|open_url"(args) {
    w.__opened = [...(w.__opened || []), args.url];
    return null;
  },

  save_download(args) {
    w.__saved = [...(w.__saved || []), { name: args.fileName, text: args.text }];
    return `C:\\Users\\alice\\Downloads\\${args.fileName}`;
  },

  save_file(args) {
    w.__saved = [...(w.__saved || []), { name: args.fileName, bytes: args.data }];
    return `C:\\Users\\alice\\Downloads\\${args.fileName}`;
  },

  share_file(args) {
    w.__shared = [...(w.__shared || []), args.fileName];
    return null;
  },

  share_text(args) {
    w.__shared = [...(w.__shared || []), args.text];
    return null;
  },

  set_bars_color: () => null,

  get_storage_warnings: () => clone(w.__STORAGE_WARNINGS__ || []),

  send_feedback(args) {
    requireAccount();
    if (w.__OLD_RELAY__) throw new Error("This sync server doesn't take messages yet");
    w.__feedback = [...(w.__feedback || []), args];
    return null;
  },
};
