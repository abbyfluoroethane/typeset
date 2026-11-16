// Per-device appearance: the Material seed color and light/dark preference.
// Kept in localStorage (not synced) since it's a property of this machine.

export type Scheme = "system" | "light" | "dark";

export const SEED_PRESETS = [
  { name: "Violet", color: "#7c4dff" },
  { name: "Tomato", color: "#e5484d" },
  { name: "Tangerine", color: "#f76b15" },
  { name: "Marigold", color: "#ffb224" },
  { name: "Fern", color: "#30a46c" },
  { name: "Teal", color: "#12a594" },
  { name: "Azure", color: "#0090ff" },
  { name: "Orchid", color: "#d6409f" },
] as const;

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Storage unavailable; the choice just won't persist.
  }
}

class Theme {
  seed = $state(read("typeset.seed") ?? SEED_PRESETS[0].color);
  scheme = $state<Scheme>((read("typeset.scheme") as Scheme | null) ?? "system");

  /** Applies the current choice to <html>; call once on startup. */
  apply() {
    const root = document.documentElement;
    root.style.setProperty("--m3v-source", this.seed);
    if (this.scheme === "system") delete root.dataset.scheme;
    else root.dataset.scheme = this.scheme;
  }

  setSeed(color: string) {
    this.seed = color;
    write("typeset.seed", color);
    this.apply();
  }

  setScheme(scheme: Scheme) {
    this.scheme = scheme;
    write("typeset.scheme", scheme);
    this.apply();
  }
}

export const theme = new Theme();
