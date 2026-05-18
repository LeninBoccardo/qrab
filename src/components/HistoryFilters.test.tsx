import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@solidjs/testing-library";
import { HistoryFilters, type FilterValue } from "./HistoryFilters";

afterEach(() => cleanup());

// The component debounces onChange by 200 ms via window.setTimeout. Fake
// timers let the tests assert on `onChange` payloads deterministically
// without flaky sleeps.
function withFakeTimers<T>(run: () => T): T {
  vi.useFakeTimers();
  try {
    return run();
  } finally {
    vi.useRealTimers();
  }
}

describe("HistoryFilters", () => {
  it("typing in search emits a debounced onChange with the trimmed payload", () => {
    withFakeTimers(() => {
      const onChange = vi.fn();
      const { getByPlaceholderText } = render(() => (
        <HistoryFilters value={{}} onChange={onChange} />
      ));
      const input = getByPlaceholderText(
        /content contains/i,
      ) as HTMLInputElement;
      fireEvent.input(input, { target: { value: "gamma" } });
      // Debounce hasn't fired yet.
      expect(onChange).not.toHaveBeenCalled();
      vi.advanceTimersByTime(200);
      expect(onChange).toHaveBeenCalledTimes(1);
      expect(onChange).toHaveBeenCalledWith({
        search: "gamma",
        kind: undefined,
        status: undefined,
        from: undefined,
        to: undefined,
      });
    });
  });

  it("changing kind emits the QrKind value", () => {
    withFakeTimers(() => {
      const onChange = vi.fn();
      const { getByTitle } = render(() => (
        <HistoryFilters value={{}} onChange={onChange} />
      ));
      const select = getByTitle("Filter by kind") as HTMLSelectElement;
      fireEvent.change(select, { target: { value: "url" } });
      vi.advanceTimersByTime(200);
      expect(onChange).toHaveBeenCalledWith(
        expect.objectContaining({ kind: "url" }),
      );
    });
  });

  it("kind=all narrows to undefined in the emitted payload", () => {
    withFakeTimers(() => {
      const onChange = vi.fn();
      const { getByTitle } = render(() => (
        <HistoryFilters value={{ kind: "text" }} onChange={onChange} />
      ));
      const select = getByTitle("Filter by kind") as HTMLSelectElement;
      fireEvent.change(select, { target: { value: "all" } });
      vi.advanceTimersByTime(200);
      expect(onChange).toHaveBeenCalledWith(
        expect.objectContaining({ kind: undefined }),
      );
    });
  });

  it("status=opened propagates; status=all narrows to undefined", () => {
    withFakeTimers(() => {
      const onChange = vi.fn();
      const { getByTitle } = render(() => (
        <HistoryFilters value={{}} onChange={onChange} />
      ));
      const select = getByTitle("Filter by status") as HTMLSelectElement;
      fireEvent.change(select, { target: { value: "opened" } });
      vi.advanceTimersByTime(200);
      expect(onChange).toHaveBeenLastCalledWith(
        expect.objectContaining({ status: "opened" }),
      );

      fireEvent.change(select, { target: { value: "all" } });
      vi.advanceTimersByTime(200);
      expect(onChange).toHaveBeenLastCalledWith(
        expect.objectContaining({ status: undefined }),
      );
    });
  });

  it("from-date input parses as local midnight epoch ms", () => {
    withFakeTimers(() => {
      const onChange = vi.fn();
      const { getByTitle } = render(() => (
        <HistoryFilters value={{}} onChange={onChange} />
      ));
      const from = getByTitle(
        "Filter from this date (inclusive)",
      ) as HTMLInputElement;
      fireEvent.change(from, { target: { value: "2026-03-15" } });
      vi.advanceTimersByTime(200);
      const payload = onChange.mock.calls[0][0] as FilterValue;
      expect(payload.from).toBeDefined();
      const d = new Date(payload.from as number);
      expect(d.getFullYear()).toBe(2026);
      expect(d.getMonth()).toBe(2);
      expect(d.getDate()).toBe(15);
      expect(d.getHours()).toBe(0);
      expect(d.getMinutes()).toBe(0);
    });
  });

  it("to-date input parses as local end-of-day epoch ms", () => {
    withFakeTimers(() => {
      const onChange = vi.fn();
      const { getByTitle } = render(() => (
        <HistoryFilters value={{}} onChange={onChange} />
      ));
      const to = getByTitle(
        "Filter up to this date (inclusive)",
      ) as HTMLInputElement;
      fireEvent.change(to, { target: { value: "2026-03-15" } });
      vi.advanceTimersByTime(200);
      const payload = onChange.mock.calls[0][0] as FilterValue;
      expect(payload.to).toBeDefined();
      const d = new Date(payload.to as number);
      expect(d.getFullYear()).toBe(2026);
      expect(d.getMonth()).toBe(2);
      expect(d.getDate()).toBe(15);
      expect(d.getHours()).toBe(23);
      expect(d.getMinutes()).toBe(59);
    });
  });

  it("a burst of search keystrokes only emits once after the trailing debounce", () => {
    withFakeTimers(() => {
      const onChange = vi.fn();
      const { getByPlaceholderText } = render(() => (
        <HistoryFilters value={{}} onChange={onChange} />
      ));
      const input = getByPlaceholderText(
        /content contains/i,
      ) as HTMLInputElement;
      // Type g, a, m within the 200 ms window.
      fireEvent.input(input, { target: { value: "g" } });
      vi.advanceTimersByTime(50);
      fireEvent.input(input, { target: { value: "ga" } });
      vi.advanceTimersByTime(50);
      fireEvent.input(input, { target: { value: "gam" } });
      vi.advanceTimersByTime(200);
      expect(onChange).toHaveBeenCalledTimes(1);
      expect(onChange).toHaveBeenLastCalledWith(
        expect.objectContaining({ search: "gam" }),
      );
    });
  });
});
