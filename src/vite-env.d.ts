/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Default relay instead of the official one (localhost in dev), e.g. `https://ezcount.example.com`. */
  readonly VITE_EZCOUNT_SERVER?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
