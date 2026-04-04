import { onCleanup } from "solid-js";

/**
 * Returns a debounced sender function. Clears any pending timer on component cleanup.
 * @param sendFn - The function to call after the debounce delay.
 * @param delayMs - Debounce delay in milliseconds (default: 16ms, one frame).
 */
export function useVolumeDebounce(sendFn: (v: number) => void, delayMs = 16) {
  let timer: ReturnType<typeof setTimeout> | null = null;

  onCleanup(() => {
    if (timer) clearTimeout(timer);
  });

  return (value: number) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      sendFn(value);
      timer = null;
    }, delayMs);
  };
}
