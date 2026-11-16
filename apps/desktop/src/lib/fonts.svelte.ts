import { SvelteMap } from "svelte/reactivity";

const loaded = new Map<string, Promise<string | null>>();
let counter = 0;

/** URL → CSS family name of each font that finished loading. Reactive, so
 * components can read it directly instead of tracking promises. */
export const loadedFonts = new SvelteMap<string, string>();

const secure = (url: string) => url.replace(/^http:\/\//, "https://");

/**
 * Loads a font file with the FontFace API and resolves to a unique CSS
 * family name, or null if it failed. Results are cached by URL.
 */
export function loadFont(url: string): Promise<string | null> {
  const key = secure(url);
  let p = loaded.get(key);
  if (!p) {
    const name = `ts-preview-${counter++}`;
    p = new FontFace(name, `url(${JSON.stringify(key)})`)
      .load()
      .then((face) => {
        document.fonts.add(face);
        loadedFonts.set(key, name);
        return name;
      })
      .catch(() => {
        // Forget failures so a later attempt (e.g. after going online) retries.
        loaded.delete(key);
        return null;
      });
    loaded.set(key, p);
  }
  return p;
}

/** The loaded CSS family name for `url`, if it has finished loading. */
export const loadedFont = (url: string | undefined) => (url ? loadedFonts.get(secure(url)) : undefined);

export function variantWeight(variant: string): number {
  const m = /^(\d+)/.exec(variant);
  return m ? Number(m[1]) : 400;
}

export function variantItalic(variant: string): boolean {
  return variant.endsWith("italic");
}

const WEIGHT_NAMES: Record<number, string> = {
  100: "Thin",
  200: "Extra Light",
  300: "Light",
  400: "Regular",
  500: "Medium",
  600: "Semi Bold",
  700: "Bold",
  800: "Extra Bold",
  900: "Black",
};

export function weightName(weight: number): string {
  return WEIGHT_NAMES[weight] ?? String(weight);
}

export function variantLabel(variant: string): string {
  const italic = variantItalic(variant);
  const w = variantWeight(variant);
  const name = weightName(w);
  if (italic) return variant === "italic" ? "Italic" : `${name} Italic`;
  return name;
}

/** Sorts variants by weight, upright before italic. */
export function sortVariants(variants: string[]): string[] {
  return [...variants].sort(
    (a, b) =>
      variantWeight(a) - variantWeight(b) || Number(variantItalic(a)) - Number(variantItalic(b)),
  );
}

export function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString();
}

export const SAMPLE_TEXT = "The quick brown fox jumps over the lazy dog";
