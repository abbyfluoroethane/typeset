// In-memory stand-in for the Tauri backend, used only by `pnpm dev` in a
// plain browser so the UI can be previewed without the app or a server.
// The catalog comes from src/lib/mock-catalog.json (a gitignored snapshot).

import type {
  CatalogResponse,
  Collection,
  CollectionInput,
  InstalledFamily,
  LibraryEntry,
  LibraryEntryInput,
  LocalState,
  ReconcileReport,
  Settings,
  Upload,
  User,
} from "./api";
import { parseKey } from "./api";

const now = () => Math.floor(Date.now() / 1000);
const delay = (ms = 150) => new Promise((r) => setTimeout(r, ms));

let rev = 0;
let user: User | null = { id: "mock", email: "you@example.com", name: "Mock User" };
let settings: Settings = { serverUrl: "https://fonts.example.com", autoInstall: true };
let lastSyncedAt: number | null = null;
let catalog: CatalogResponse | null = null;
const library = new Map<string, LibraryEntry>();
const collections = new Map<string, Collection>();
const uploads = new Map<string, Upload>();
const installed = new Map<string, InstalledFamily>();

function putEntry(key: string, input: LibraryEntryInput): LibraryEntry {
  const parsed = parseKey(key);
  if (!parsed) throw `invalid family key: ${key}`;
  const entry: LibraryEntry = {
    key,
    source: parsed.source,
    family: parsed.family,
    variants: input.variants,
    favorite: input.favorite,
    deleted: false,
    rev: ++rev,
    updatedAt: now(),
  };
  library.set(key, entry);
  return entry;
}

function install(key: string): InstalledFamily {
  const parsed = parseKey(key);
  if (!parsed) throw `invalid family key: ${key}`;
  const entry = library.get(key);
  const fam = catalog?.families.find((f) => f.family === parsed.family);
  const variants =
    parsed.source === "google"
      ? entry?.variants.length
        ? entry.variants
        : (fam?.variants ?? ["regular"])
      : [...uploads.values()].filter((u) => u.family === parsed.family).map((u) => u.id);
  const family: InstalledFamily = {
    key,
    family: parsed.family,
    source: parsed.source,
    files: variants.map((variant) => ({
      variant,
      path: `~/Library/Fonts/Typeset/${parsed.family.replace(/\W/g, "")}-${variant}.ttf`,
      version: fam?.version ?? "upload",
    })),
    installedAt: now(),
  };
  installed.set(key, family);
  return family;
}

function reconcile(): ReconcileReport {
  const report: ReconcileReport = { installed: [], updated: [], removed: [], failed: [] };
  for (const entry of library.values()) {
    if (entry.deleted && installed.delete(entry.key)) report.removed.push(entry.key);
    if (!entry.deleted && !installed.has(entry.key)) {
      install(entry.key);
      report.installed.push(entry.key);
    }
  }
  return report;
}

function localState(): LocalState {
  return {
    library: [...library.values()]
      .filter((e) => !e.deleted)
      .sort((a, b) => a.family.localeCompare(b.family)),
    collections: [...collections.values()]
      .filter((c) => !c.deleted)
      .sort((a, b) => a.name.localeCompare(b.name)),
    uploads: [...uploads.values()].filter((u) => !u.deleted),
    lastRev: rev,
    lastSyncedAt,
  };
}

function seed() {
  for (const family of ["Inter", "Roboto Flex", "IBM Plex Mono", "Fraunces"]) {
    putEntry(`google:${family}`, { variants: [], favorite: family === "Inter" });
  }
  putEntry("upload:Studio Grotesk", { variants: [], favorite: false });
  for (const [style, weight, italic] of [
    ["Regular", 400, false],
    ["Bold", 700, false],
    ["Italic", 400, true],
  ] as const) {
    const id = crypto.randomUUID();
    uploads.set(id, {
      id,
      family: "Studio Grotesk",
      style,
      weight,
      italic,
      filename: `StudioGrotesk-${style}.otf`,
      sha256: id.replace(/-/g, ""),
      size: 84_000,
      deleted: false,
      rev: ++rev,
      updatedAt: now(),
    });
  }
  const id = crypto.randomUUID();
  collections.set(id, {
    id,
    name: "Editorial",
    familyKeys: ["google:Fraunces", "google:Inter"],
    deleted: false,
    rev: ++rev,
    updatedAt: now(),
  });
  install("google:Inter");
  install("google:IBM Plex Mono");
}

const handlers: Record<string, (args: Record<string, any>) => unknown> = {
  login: () => (user = { id: "mock", email: "you@example.com", name: "Mock User" }),
  logout: () => void (user = null),
  current_user: () => user,
  get_settings: () => settings,
  update_settings: ({ settings: s }) => (settings = s),
  get_catalog: async () => {
    catalog ??= (await import("./mock-catalog.json")).default as CatalogResponse;
    return catalog;
  },
  get_local_state: localState,
  sync_now: () => {
    lastSyncedAt = now();
    return { state: localState(), reconcile: settings.autoInstall ? reconcile() : null };
  },
  library_put: ({ key, input }) => putEntry(key, input),
  library_delete: ({ key }) => {
    const entry = library.get(key);
    if (!entry) throw "not found";
    entry.deleted = true;
    entry.rev = ++rev;
    return entry;
  },
  collection_put: ({ id, input }) => {
    const c: Collection = {
      id,
      ...(input as CollectionInput),
      deleted: false,
      rev: ++rev,
      updatedAt: now(),
    };
    collections.set(id, c);
    return c;
  },
  collection_delete: ({ id }) => {
    const c = collections.get(id);
    if (!c) throw "not found";
    c.deleted = true;
    return c;
  },
  upload_fonts: () => {
    throw "Uploading needs the desktop app (file dialog isn't available in the browser mock)";
  },
  delete_upload: ({ id }) => {
    const u = uploads.get(id);
    if (!u) throw "not found";
    u.deleted = true;
    return u;
  },
  list_installed: () => [...installed.values()],
  install_family: ({ key }) => install(key),
  uninstall_family: ({ key }) => void installed.delete(key),
  reconcile_fonts: reconcile,
};

seed();

export async function mockInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  await delay();
  const handler = handlers[cmd];
  if (!handler) throw `mock: unknown command ${cmd}`;
  // Return copies so UI state never aliases the mock's maps.
  return structuredClone(await handler(args)) as T;
}
