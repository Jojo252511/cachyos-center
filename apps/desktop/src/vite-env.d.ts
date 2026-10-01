/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Set by the Tauri CLI when the frontend is built or served for the desktop app. */
  readonly TAURI_ENV_PLATFORM?: string;
  readonly TAURI_ENV_DEBUG?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
