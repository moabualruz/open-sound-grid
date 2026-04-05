import { describe, it, expect } from "vitest";
import { computeBackoffDelay, BACKOFF_INITIAL_MS, BACKOFF_CAP_MS } from "./sessionStore";

describe("computeBackoffDelay", () => {
  it("returns 1000ms for attempt 0", () => {
    expect(computeBackoffDelay(0)).toBe(1000);
  });

  it("returns 2000ms for attempt 1", () => {
    expect(computeBackoffDelay(1)).toBe(2000);
  });

  it("returns 4000ms for attempt 2", () => {
    expect(computeBackoffDelay(2)).toBe(4000);
  });

  it("returns 8000ms for attempt 3", () => {
    expect(computeBackoffDelay(3)).toBe(8000);
  });

  it("returns 16000ms for attempt 4", () => {
    expect(computeBackoffDelay(4)).toBe(16000);
  });

  it("caps at 30000ms for attempt 5", () => {
    expect(computeBackoffDelay(5)).toBe(30000);
  });

  it("caps at 30000ms for attempt 10", () => {
    expect(computeBackoffDelay(10)).toBe(30000);
  });

  it("caps at 30000ms for large attempt numbers", () => {
    expect(computeBackoffDelay(100)).toBe(30000);
  });
});

describe("BACKOFF_INITIAL_MS", () => {
  it("is 1000", () => {
    expect(BACKOFF_INITIAL_MS).toBe(1000);
  });
});

describe("BACKOFF_CAP_MS", () => {
  it("is 30000", () => {
    expect(BACKOFF_CAP_MS).toBe(30000);
  });
});
