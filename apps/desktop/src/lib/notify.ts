// Thin wrapper over the Material snackbar so app code doesn't depend on its
// positional signature. Errors stay up until dismissed.

import { snackbar } from "m3-svelte";

type Actions = Record<string, () => void>;

export const notify = {
  success: (message: string, actions?: Actions) => snackbar(message, actions),
  warning: (message: string, actions?: Actions) => snackbar(message, actions, true, 8000),
  error: (message: string, actions?: Actions) => snackbar(message, actions, true, -1),
};
