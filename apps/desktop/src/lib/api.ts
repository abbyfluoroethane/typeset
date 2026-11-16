import { invoke as tauriInvoke } from "@tauri-apps/api/core";

// Outside Tauri in dev (`pnpm dev` in a browser), use the in-memory mock so
// the UI can be previewed. Production builds always call the real backend.
const useMock = import.meta.env.DEV && !("__TAURI_INTERNALS__" in window);

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (useMock) return (await import("./mock")).mockInvoke<T>(cmd, args);
  return tauriInvoke<T>(cmd, args);
}

export type FontSource = "google" | "upload";

export interface CatalogFamily {
  family: string;
  category: string;
  variants: string[];
  subsets: string[];
  version: string;
  lastModified: string;
  files: Record<string, string>;
  menu: string | null;
}

export interface CatalogResponse {
  fetchedAt: number;
  families: CatalogFamily[];
}

export interface User {
  id: string;
  email: string | null;
  name: string | null;
}

export interface LibraryEntry {
  key: string;
  source: FontSource;
  family: string;
  variants: string[];
  favorite: boolean;
  deleted: boolean;
  rev: number;
  updatedAt: number;
}

export interface LibraryEntryInput {
  variants: string[];
  favorite: boolean;
}

export interface Collection {
  id: string;
  name: string;
  familyKeys: string[];
  deleted: boolean;
  rev: number;
  updatedAt: number;
}

export interface CollectionInput {
  name: string;
  familyKeys: string[];
}

export interface Upload {
  id: string;
  family: string;
  style: string;
  weight: number;
  italic: boolean;
  filename: string;
  sha256: string;
  size: number;
  deleted: boolean;
  rev: number;
  updatedAt: number;
}

export interface Settings {
  serverUrl: string | null;
  autoInstall: boolean;
}

export interface LocalState {
  library: LibraryEntry[];
  collections: Collection[];
  uploads: Upload[];
  lastRev: number;
  lastSyncedAt: number | null;
}

export interface ReconcileFailure {
  key: string;
  error: string;
}

export interface ReconcileReport {
  installed: string[];
  updated: string[];
  removed: string[];
  failed: ReconcileFailure[];
}

export interface SyncSummary {
  state: LocalState;
  reconcile: ReconcileReport | null;
}

export interface InstalledFile {
  variant: string;
  path: string;
  version: string;
}

export interface InstalledFamily {
  key: string;
  family: string;
  source: FontSource;
  files: InstalledFile[];
  installedAt: number;
}

export function parseKey(key: string): { source: FontSource; family: string } | null {
  const i = key.indexOf(":");
  if (i < 0) return null;
  const source = key.slice(0, i);
  const family = key.slice(i + 1);
  if ((source !== "google" && source !== "upload") || !family) return null;
  return { source, family };
}

/** Tauri rejects with a plain string; normalise anything else. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

export const familyKey = (source: FontSource, family: string) => `${source}:${family}`;

export const login = () => invoke<User>("login");
export const logout = () => invoke<void>("logout");
export const currentUser = () => invoke<User | null>("current_user");
export const getSettings = () => invoke<Settings>("get_settings");
export const updateSettings = (settings: Settings) =>
  invoke<Settings>("update_settings", { settings });
export const getCatalog = (refresh: boolean) => invoke<CatalogResponse>("get_catalog", { refresh });
export const getLocalState = () => invoke<LocalState>("get_local_state");
export const syncNow = () => invoke<SyncSummary>("sync_now");
export const libraryPut = (key: string, input: LibraryEntryInput) =>
  invoke<LibraryEntry>("library_put", { key, input });
export const libraryDelete = (key: string) => invoke<LibraryEntry>("library_delete", { key });
export const collectionPut = (id: string, input: CollectionInput) =>
  invoke<Collection>("collection_put", { id, input });
export const collectionDelete = (id: string) => invoke<Collection>("collection_delete", { id });
export const uploadFonts = (paths: string[]) => invoke<Upload[]>("upload_fonts", { paths });
export const deleteUpload = (id: string) => invoke<Upload>("delete_upload", { id });
export const listInstalled = () => invoke<InstalledFamily[]>("list_installed");
export const installFamily = (key: string) => invoke<InstalledFamily>("install_family", { key });
export const uninstallFamily = (key: string) => invoke<void>("uninstall_family", { key });
export const reconcileFonts = () => invoke<ReconcileReport>("reconcile_fonts");
