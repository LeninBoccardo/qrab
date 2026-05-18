import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { ConfirmOpenAll } from "./ConfirmOpenAll";
import type { ScanRow } from "../lib/types";

// Kobalte Dialog renders its content in a Portal mounted on
// `document.body`, so screen-level queries (not the per-container ones
// from `render`) are required to see the dialog.

function urlRow(id: number, content: string): ScanRow {
  return {
    id,
    batchId: "B",
    content,
    kind: "url",
    monitorIndex: 0,
    scannedAt: 1,
    opened: false,
    openedAt: null,
    copied: false,
    copiedAt: null,
  };
}

afterEach(() => cleanup());

describe("ConfirmOpenAll", () => {
  it("does not render content when closed", () => {
    render(() => (
      <ConfirmOpenAll
        open={false}
        onOpenChange={() => {}}
        rows={[urlRow(1, "https://a.test"), urlRow(2, "https://b.test")]}
        skippedNonUrl={0}
        onConfirm={() => {}}
      />
    ));
    expect(screen.queryByText(/Open 2 URLs/i)).toBeNull();
    expect(screen.queryByText("https://a.test")).toBeNull();
  });

  it("renders every URL when open — not just the count", () => {
    render(() => (
      <ConfirmOpenAll
        open={true}
        onOpenChange={() => {}}
        rows={[
          urlRow(1, "https://alpha.test"),
          urlRow(2, "https://beta.test"),
          urlRow(3, "https://gamma.test"),
          urlRow(4, "https://delta.test"),
        ]}
        skippedNonUrl={0}
        onConfirm={() => {}}
      />
    ));
    expect(screen.getByText("https://alpha.test")).toBeInTheDocument();
    expect(screen.getByText("https://beta.test")).toBeInTheDocument();
    expect(screen.getByText("https://gamma.test")).toBeInTheDocument();
    expect(screen.getByText("https://delta.test")).toBeInTheDocument();
  });

  it("pluralizes the title — singular for 1 URL, plural for 2+", () => {
    const { unmount } = render(() => (
      <ConfirmOpenAll
        open={true}
        onOpenChange={() => {}}
        rows={[urlRow(1, "https://only.test")]}
        skippedNonUrl={0}
        onConfirm={() => {}}
      />
    ));
    expect(screen.getByRole("heading").textContent).toMatch(/Open 1 URL\?/);
    unmount();

    render(() => (
      <ConfirmOpenAll
        open={true}
        onOpenChange={() => {}}
        rows={[urlRow(1, "a"), urlRow(2, "b")]}
        skippedNonUrl={0}
        onConfirm={() => {}}
      />
    ));
    expect(screen.getByRole("heading").textContent).toMatch(/Open 2 URLs\?/);
  });

  it("Cancel button calls onOpenChange(false), not onConfirm", () => {
    const onOpenChange = vi.fn();
    const onConfirm = vi.fn();
    render(() => (
      <ConfirmOpenAll
        open={true}
        onOpenChange={onOpenChange}
        rows={[urlRow(1, "https://a.test"), urlRow(2, "https://b.test")]}
        skippedNonUrl={0}
        onConfirm={onConfirm}
      />
    ));
    fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("primary button calls onConfirm and shows the count", () => {
    const onConfirm = vi.fn();
    render(() => (
      <ConfirmOpenAll
        open={true}
        onOpenChange={() => {}}
        rows={[urlRow(1, "a"), urlRow(2, "b"), urlRow(3, "c")]}
        skippedNonUrl={0}
        onConfirm={onConfirm}
      />
    ));
    fireEvent.click(screen.getByRole("button", { name: /Open all 3/i }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("shows the skipped-non-URL footnote only when > 0", () => {
    const { unmount } = render(() => (
      <ConfirmOpenAll
        open={true}
        onOpenChange={() => {}}
        rows={[urlRow(1, "a")]}
        skippedNonUrl={0}
        onConfirm={() => {}}
      />
    ));
    expect(screen.queryByText(/non-URL/i)).toBeNull();
    unmount();

    render(() => (
      <ConfirmOpenAll
        open={true}
        onOpenChange={() => {}}
        rows={[urlRow(1, "a")]}
        skippedNonUrl={2}
        onConfirm={() => {}}
      />
    ));
    // The footnote is rendered with String children separated by JSX
    // whitespace, so the text node tree contains "+2", "non-URL", and
    // "items will not be opened" as separate runs. Walk every <p> and
    // compare the collapsed normalized text — there are multiple <p>s
    // in the dialog (Description + footnote), so we scan rather than
    // assume order.
    const paragraphs = Array.from(document.body.querySelectorAll("p"));
    const matched = paragraphs.some((p) =>
      /\+2 non-URL items will not be opened/i.test(
        (p.textContent ?? "").replace(/\s+/g, " "),
      ),
    );
    expect(matched).toBe(true);
  });
});
