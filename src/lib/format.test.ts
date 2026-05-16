import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { absoluteTime, formatError, relativeTime } from "./format";

describe("relativeTime", () => {
  const NOW = new Date("2026-05-15T12:00:00.000Z").getTime();

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns "just now" within the last minute', () => {
    expect(relativeTime(NOW - 30_000)).toBe("just now");
    expect(relativeTime(NOW - 59_999)).toBe("just now");
  });

  it("returns minutes between 1 minute and 1 hour", () => {
    expect(relativeTime(NOW - 60_000)).toBe("1m ago");
    expect(relativeTime(NOW - 30 * 60_000)).toBe("30m ago");
    expect(relativeTime(NOW - 59 * 60_000)).toBe("59m ago");
  });

  it("returns hours between 1 hour and 1 day", () => {
    expect(relativeTime(NOW - 60 * 60_000)).toBe("1h ago");
    expect(relativeTime(NOW - 5 * 60 * 60_000)).toBe("5h ago");
  });

  it("returns days between 1 day and 1 week", () => {
    expect(relativeTime(NOW - 24 * 60 * 60_000)).toBe("1d ago");
    expect(relativeTime(NOW - 6 * 24 * 60 * 60_000)).toBe("6d ago");
  });

  it("falls back to a locale date for older entries", () => {
    const stamp = NOW - 14 * 24 * 60 * 60_000;
    expect(relativeTime(stamp)).toBe(new Date(stamp).toLocaleDateString());
  });
});

describe("absoluteTime", () => {
  it("returns the full locale string for a given epoch ms", () => {
    const stamp = new Date("2026-05-15T12:00:00.000Z").getTime();
    expect(absoluteTime(stamp)).toBe(new Date(stamp).toLocaleString());
  });
});

describe("formatError", () => {
  it("returns string values as-is", () => {
    expect(formatError("boom")).toBe("boom");
  });

  it("uses .message on Error instances", () => {
    expect(formatError(new Error("oops"))).toBe("oops");
    expect(formatError(new TypeError("bad type"))).toBe("bad type");
  });

  it("stringifies other values", () => {
    expect(formatError(42)).toBe("42");
    expect(formatError(null)).toBe("null");
    expect(formatError(undefined)).toBe("undefined");
    expect(formatError({ a: 1 })).toBe("[object Object]");
  });
});
