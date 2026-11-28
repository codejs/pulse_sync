import { describe, expect, it } from "vitest";
import { formatBytes, formatRelative } from "./format";

describe("formatBytes", () => {
  it.each([
    [0, "0 B"],
    [1023, "1023 B"],
    [1024, "1.0 KB"],
    [1536, "1.5 KB"],
    [150 * 1024 * 1024, "150 MB"],
    [5 * 1024 ** 4, "5.0 TB"],
  ])("%d -> %s", (input, expected) => {
    expect(formatBytes(input)).toBe(expected);
  });

  it("rejects nonsense", () => {
    expect(formatBytes(-1)).toBe("—");
    expect(formatBytes(Number.NaN)).toBe("—");
  });
});

describe("formatRelative", () => {
  const now = Date.UTC(2026, 0, 1);
  it("handles missing timestamps", () => {
    expect(formatRelative(null, now)).toBe("never");
  });
  it("picks a sensible unit", () => {
    expect(formatRelative(now - 5_000, now)).toMatch(/5 seconds ago/);
    expect(formatRelative(now - 3 * 3_600_000, now)).toMatch(/3 hours ago/);
  });
});
