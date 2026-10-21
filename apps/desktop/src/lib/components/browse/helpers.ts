import { snackbar } from "m3-svelte";
import { app } from "$lib/state.svelte";

export const prettyCategory = (s: string) => s.replace(/-/g, " ").replace(/^./, (c) => c.toUpperCase());

/** Removes a family from the library and offers Undo in a snackbar. */
export async function removeWithUndo(key: string, name: string) {
  const entry = app.libraryByKey.get(key);
  await app.removeFromLibrary(key);
  if (entry && !app.libraryByKey.has(key)) {
    snackbar(`Removed ${name} from library`, {
      Undo: () => void app.putLibrary(key, entry.variants, entry.favorite),
    });
  }
}
