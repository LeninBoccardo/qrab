import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@solidjs/testing-library";
import { RegionSelector } from "./RegionSelector";

// jsdom doesn't lay out elements, so getBoundingClientRect() returns
// zeros by default. The RegionSelector divides by rect dimensions to
// convert CSS pixels to image pixels — we need a non-zero, known rect.
function stubImageRect(width = 100, height = 100): void {
  Element.prototype.getBoundingClientRect = vi.fn(() => ({
    width,
    height,
    top: 0,
    left: 0,
    right: width,
    bottom: height,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  })) as unknown as () => DOMRect;
}

beforeEach(() => stubImageRect());
afterEach(() => cleanup());

describe("RegionSelector", () => {
  it("renders Decode disabled and Cancel enabled before any drag", () => {
    const onDecode = vi.fn();
    const onCancel = vi.fn();
    const { getByRole } = render(() => (
      <RegionSelector
        imageSrc=""
        imageWidth={1000}
        imageHeight={1000}
        onDecode={onDecode}
        onCancel={onCancel}
      />
    ));
    expect(getByRole("button", { name: /decode/i })).toBeDisabled();
    expect(getByRole("button", { name: /cancel/i })).not.toBeDisabled();
  });

  it("dragging a sufficient rectangle enables Decode and yields image-pixel bounds", () => {
    const onDecode = vi.fn();
    const onCancel = vi.fn();
    const { container, getByRole } = render(() => (
      <RegionSelector
        imageSrc=""
        imageWidth={1000}
        imageHeight={1000}
        onDecode={onDecode}
        onCancel={onCancel}
      />
    ));
    const img = container.querySelector("img");
    if (!img) throw new Error("img not found");
    // CSS rect is 100x100, image is 1000x1000 → scale 10. Drag from
    // (10,10) to (50,50) in CSS px → bounds {x:100, y:100, w:400, h:400}
    // in image px.
    fireEvent.mouseDown(img, { clientX: 10, clientY: 10, button: 0 });
    fireEvent.mouseMove(img.parentElement!, { clientX: 50, clientY: 50 });
    fireEvent.mouseUp(img.parentElement!);

    const decode = getByRole("button", { name: /decode/i });
    expect(decode).not.toBeDisabled();
    fireEvent.click(decode);
    expect(onDecode).toHaveBeenCalledTimes(1);
    expect(onDecode).toHaveBeenCalledWith({
      x: 100,
      y: 100,
      w: 400,
      h: 400,
    });
  });

  it("Esc on the window invokes onCancel", () => {
    const onDecode = vi.fn();
    const onCancel = vi.fn();
    render(() => (
      <RegionSelector
        imageSrc=""
        imageWidth={1000}
        imageHeight={1000}
        onDecode={onDecode}
        onCancel={onCancel}
      />
    ));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("Enter on the window with valid bounds invokes onDecode", () => {
    const onDecode = vi.fn();
    const onCancel = vi.fn();
    const { container } = render(() => (
      <RegionSelector
        imageSrc=""
        imageWidth={1000}
        imageHeight={1000}
        onDecode={onDecode}
        onCancel={onCancel}
      />
    ));
    const img = container.querySelector("img");
    if (!img) throw new Error("img not found");
    fireEvent.mouseDown(img, { clientX: 10, clientY: 10, button: 0 });
    fireEvent.mouseMove(img.parentElement!, { clientX: 50, clientY: 50 });
    fireEvent.mouseUp(img.parentElement!);

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    expect(onDecode).toHaveBeenCalledTimes(1);
    expect(onDecode).toHaveBeenCalledWith({
      x: 100,
      y: 100,
      w: 400,
      h: 400,
    });
  });

  it("does not enable Decode when the dragged rect is below MIN_SIDE_PX in image space", () => {
    const onDecode = vi.fn();
    const onCancel = vi.fn();
    const { container, getByRole } = render(() => (
      <RegionSelector
        imageSrc=""
        // Image only 10px wide → scale 0.1. A 5-CSS-px drag is 0.5 image-px.
        imageWidth={10}
        imageHeight={10}
        onDecode={onDecode}
        onCancel={onCancel}
      />
    ));
    const img = container.querySelector("img");
    if (!img) throw new Error("img not found");
    fireEvent.mouseDown(img, { clientX: 10, clientY: 10, button: 0 });
    fireEvent.mouseMove(img.parentElement!, { clientX: 15, clientY: 15 });
    fireEvent.mouseUp(img.parentElement!);
    expect(getByRole("button", { name: /decode/i })).toBeDisabled();
  });

  it("Cancel button invokes onCancel", () => {
    const onDecode = vi.fn();
    const onCancel = vi.fn();
    const { getByRole } = render(() => (
      <RegionSelector
        imageSrc=""
        imageWidth={1000}
        imageHeight={1000}
        onDecode={onDecode}
        onCancel={onCancel}
      />
    ));
    fireEvent.click(getByRole("button", { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("non-left-button mousedown does not start a drag", () => {
    const onDecode = vi.fn();
    const onCancel = vi.fn();
    const { container, getByRole } = render(() => (
      <RegionSelector
        imageSrc=""
        imageWidth={1000}
        imageHeight={1000}
        onDecode={onDecode}
        onCancel={onCancel}
      />
    ));
    const img = container.querySelector("img");
    if (!img) throw new Error("img not found");
    // Right-click mousedown (button=2) should be ignored.
    fireEvent.mouseDown(img, { clientX: 10, clientY: 10, button: 2 });
    fireEvent.mouseMove(img.parentElement!, { clientX: 50, clientY: 50 });
    fireEvent.mouseUp(img.parentElement!);
    expect(getByRole("button", { name: /decode/i })).toBeDisabled();
  });
});
