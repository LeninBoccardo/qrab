// Rubber-band region selector over an `<img>`. Pure UI primitive — the
// parent (RegionSelectWindow in C9) supplies `imageSrc` plus the source
// image's native dimensions and receives the user's selection in *image
// pixels*, ready to hand to `scan_region` (C8).

import {
  Component,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { Button } from "./ui/Button";

export interface Bounds {
  x: number;
  y: number;
  w: number;
  h: number;
}

interface RegionSelectorProps {
  imageSrc: string;
  /** Native (image-pixel) width of `imageSrc`. */
  imageWidth: number;
  /** Native (image-pixel) height of `imageSrc`. */
  imageHeight: number;
  onDecode: (bounds: Bounds) => void;
  onCancel: () => void;
}

/** Minimum side length (in image pixels) for a usable selection. */
const MIN_SIDE_PX = 4;

interface Point {
  x: number;
  y: number;
}

export const RegionSelector: Component<RegionSelectorProps> = (props) => {
  const [start, setStart] = createSignal<Point | null>(null);
  const [end, setEnd] = createSignal<Point | null>(null);
  const [dragging, setDragging] = createSignal(false);
  // Re-render-trigger for layout-derived math. Bumped on window resize.
  const [layoutTick, setLayoutTick] = createSignal(0);
  let imgRef: HTMLImageElement | undefined;
  // Cached image rect for the duration of a drag. mousemove fires at the
  // browser's refresh rate, and reading getBoundingClientRect on each event
  // is cheap individually but still adds up — and any DOM mutation between
  // events would force a layout flush. We capture once on mousedown and
  // reuse for the whole drag; the image doesn't move while the user drags.
  let dragRect: DOMRect | null = null;

  const bounds = createMemo<Bounds | null>(() => {
    const a = start();
    const b = end();
    if (!a || !b) return null;
    const x = Math.min(a.x, b.x);
    const y = Math.min(a.y, b.y);
    const w = Math.abs(a.x - b.x);
    const h = Math.abs(a.y - b.y);
    if (w < MIN_SIDE_PX || h < MIN_SIDE_PX) return null;
    return { x, y, w, h };
  });

  function rectToImageCoords(
    rect: DOMRect,
    clientX: number,
    clientY: number,
  ): Point | null {
    if (rect.width === 0 || rect.height === 0) return null;
    const cssX = clamp(clientX - rect.left, 0, rect.width);
    const cssY = clamp(clientY - rect.top, 0, rect.height);
    const scaleX = props.imageWidth / rect.width;
    const scaleY = props.imageHeight / rect.height;
    return { x: Math.round(cssX * scaleX), y: Math.round(cssY * scaleY) };
  }

  function onMouseDown(e: MouseEvent): void {
    if (e.button !== 0) return;
    if (!imgRef) return;
    dragRect = imgRef.getBoundingClientRect();
    const p = rectToImageCoords(dragRect, e.clientX, e.clientY);
    if (!p) {
      dragRect = null;
      return;
    }
    setStart(p);
    setEnd(p);
    setDragging(true);
  }

  function onMouseMove(e: MouseEvent): void {
    if (!dragging() || !dragRect) return;
    const p = rectToImageCoords(dragRect, e.clientX, e.clientY);
    if (p) setEnd(p);
  }

  function onMouseUp(): void {
    setDragging(false);
    dragRect = null;
  }

  function onKeyDown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      props.onCancel();
    } else if (e.key === "Enter" && bounds()) {
      e.preventDefault();
      const b = bounds();
      if (b) props.onDecode(b);
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onKeyDown);
    const onResize = (): void => {
      setLayoutTick((n) => n + 1);
    };
    window.addEventListener("resize", onResize);
    onCleanup(() => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", onResize);
    });
  });

  return (
    <div class="flex h-full w-full select-none flex-col bg-neutral-950 text-neutral-100">
      <div
        class="relative flex flex-1 items-center justify-center overflow-hidden"
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
      >
        <img
          ref={imgRef}
          src={props.imageSrc}
          class="max-h-full max-w-full cursor-crosshair select-none"
          draggable={false}
          alt=""
          onMouseDown={onMouseDown}
          onLoad={() => setLayoutTick((n) => n + 1)}
        />
        <Show when={imgRef && bounds()}>
          <SelectionOverlay
            getImg={() => imgRef}
            // Read layoutTick so this re-evaluates on resize/load.
            tick={layoutTick()}
            bounds={bounds()!}
            imageWidth={props.imageWidth}
            imageHeight={props.imageHeight}
          />
        </Show>
      </div>
      <div class="flex items-center justify-between gap-3 border-t border-neutral-800 bg-neutral-950 px-4 py-2 text-xs text-neutral-400">
        <span>
          Drag a rectangle around the QR. Esc to cancel, Enter to decode.
        </span>
        <div class="flex gap-2">
          <Button variant="ghost" onClick={() => props.onCancel()}>
            Cancel
          </Button>
          <Button
            variant="primary"
            disabled={!bounds()}
            onClick={() => {
              const b = bounds();
              if (b) props.onDecode(b);
            }}
          >
            Decode
          </Button>
        </div>
      </div>
    </div>
  );
};

interface SelectionOverlayProps {
  getImg: () => HTMLImageElement | undefined;
  tick: number;
  bounds: Bounds;
  imageWidth: number;
  imageHeight: number;
}

/**
 * SVG overlay positioned exactly over the rendered `<img>`. The mask
 * darkens everything outside the selection while the stroke rect outlines
 * it. CSS-pixel math is derived from `getBoundingClientRect()` — the
 * `tick` prop is read so the memo re-runs on resize/image-load.
 */
const SelectionOverlay: Component<SelectionOverlayProps> = (props) => {
  const layout = createMemo(() => {
    // Touch tick so Solid re-runs this on resize.
    void props.tick;
    const img = props.getImg();
    if (!img || !img.parentElement) return null;
    const imgRect = img.getBoundingClientRect();
    const containerRect = img.parentElement.getBoundingClientRect();
    if (imgRect.width === 0 || imgRect.height === 0) return null;
    const scaleX = imgRect.width / props.imageWidth;
    const scaleY = imgRect.height / props.imageHeight;
    return {
      // SVG positioning relative to the container
      left: imgRect.left - containerRect.left,
      top: imgRect.top - containerRect.top,
      width: imgRect.width,
      height: imgRect.height,
      // Selection rect within the SVG (CSS px relative to the image)
      rectX: props.bounds.x * scaleX,
      rectY: props.bounds.y * scaleY,
      rectW: props.bounds.w * scaleX,
      rectH: props.bounds.h * scaleY,
    };
  });

  return (
    <Show when={layout()}>
      {(l) => (
        <svg
          class="pointer-events-none absolute"
          style={{
            left: `${l().left}px`,
            top: `${l().top}px`,
            width: `${l().width}px`,
            height: `${l().height}px`,
          }}
        >
          <defs>
            <mask id="qrab-region-mask">
              <rect width="100%" height="100%" fill="white" />
              <rect
                x={l().rectX}
                y={l().rectY}
                width={l().rectW}
                height={l().rectH}
                fill="black"
              />
            </mask>
          </defs>
          <rect
            width="100%"
            height="100%"
            fill="rgba(0,0,0,0.55)"
            mask="url(#qrab-region-mask)"
          />
          <rect
            x={l().rectX}
            y={l().rectY}
            width={l().rectW}
            height={l().rectH}
            fill="none"
            stroke="rgb(96,165,250)"
            stroke-width={2}
            shape-rendering="crispEdges"
          />
        </svg>
      )}
    </Show>
  );
};

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(Math.max(v, lo), hi);
}
