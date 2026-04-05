export const BACKOFF_INITIAL_MS = 1000;
export const BACKOFF_CAP_MS = 30_000;

/** Returns the delay in ms for the given attempt index (0-based). */
export function computeBackoffDelay(attempt: number): number {
  const delay = BACKOFF_INITIAL_MS * Math.pow(2, attempt);
  return Math.min(delay, BACKOFF_CAP_MS);
}
