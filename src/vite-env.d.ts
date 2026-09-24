/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Relay URL pre-filled on the login screen, e.g. `https://ezcount.example.com`. */
  readonly VITE_EZCOUNT_SERVER?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
