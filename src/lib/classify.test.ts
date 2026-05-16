import { describe, expect, it } from "vitest";
import { isOpenable, kindLabel } from "./classify";
import type { QrKind } from "./types";

describe("kindLabel", () => {
  it("returns a human label for every QrKind variant", () => {
    const labels: Record<QrKind, string> = {
      url: "Link",
      text: "Text",
      wifi: "Wi-Fi",
      vcard: "Contact",
      email: "Email",
      phone: "Phone",
      other: "Other",
    };
    for (const [kind, expected] of Object.entries(labels) as [
      QrKind,
      string,
    ][]) {
      expect(kindLabel(kind)).toBe(expected);
    }
  });
});

describe("isOpenable", () => {
  it("returns true only for url kind", () => {
    expect(isOpenable("url")).toBe(true);
    expect(isOpenable("text")).toBe(false);
    expect(isOpenable("wifi")).toBe(false);
    expect(isOpenable("vcard")).toBe(false);
    expect(isOpenable("email")).toBe(false);
    expect(isOpenable("phone")).toBe(false);
    expect(isOpenable("other")).toBe(false);
  });
});
