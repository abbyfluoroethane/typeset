import type { Action } from "svelte/action";

/** Calls `onVisible` once, the first time the element scrolls into view. */
export const inView: Action<HTMLElement, () => void> = (node, onVisible) => {
  const io = new IntersectionObserver(
    (entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        onVisible?.();
        io.disconnect();
      }
    },
    { rootMargin: "200px" },
  );
  io.observe(node);
  return { destroy: () => io.disconnect() };
};

/** Calls `onVisible` every time the element enters view (infinite scroll sentinel). */
export const onEnter: Action<HTMLElement, () => void> = (node, onVisible) => {
  let cb = onVisible;
  const io = new IntersectionObserver(
    (entries) => {
      if (entries.some((e) => e.isIntersecting)) cb?.();
    },
    { rootMargin: "600px" },
  );
  io.observe(node);
  return {
    update: (next) => (cb = next),
    destroy: () => io.disconnect(),
  };
};
