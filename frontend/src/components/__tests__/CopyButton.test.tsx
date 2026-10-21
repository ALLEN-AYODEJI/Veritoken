import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { CopyButton } from "../CopyButton";

beforeEach(() => {
  Object.assign(navigator, {
    clipboard: {
      writeText: vi.fn().mockResolvedValue(undefined),
    },
  });
});

describe("CopyButton", () => {
  it("renders with the correct label", () => {
    render(<CopyButton text="hello" />);
    expect(screen.getByRole("button")).toBeDefined();
  });

  it("copies text on click", async () => {
    render(<CopyButton text="hello world" />);
    const btn = screen.getByRole("button");

    await act(async () => {
      fireEvent.click(btn);
    });

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("hello world");
  });

  it("shows Copied! after clicking", async () => {
    render(<CopyButton text="test" />);
    const btn = screen.getByRole("button");

    expect(btn.textContent).toBe("Copy");

    await act(async () => {
      fireEvent.click(btn);
    });

    expect(btn.textContent).toBe("Copied!");
  });

  it("uses the provided aria-label", () => {
    render(<CopyButton text="abc123" label="Copy transaction hash" />);
    expect(
      screen.getByRole("button", { name: "Copy transaction hash" }),
    ).toBeDefined();
  });

  it("falls back to a default aria-label", () => {
    render(<CopyButton text="GABCDEF" />);
    expect(
      screen.getByRole("button", { name: "Copy GABCDEF" }),
    ).toBeDefined();
  });

  it("handles clipboard failure gracefully", async () => {
    vi.mocked(navigator.clipboard.writeText).mockRejectedValueOnce(new Error("denied"));

    render(<CopyButton text="fail" />);
    const btn = screen.getByRole("button");

    await act(async () => {
      fireEvent.click(btn);
    });

    expect(btn.textContent).toBe("Copy");
  });

  it("does not show success when clipboard write is rejected", async () => {
    vi.mocked(navigator.clipboard.writeText).mockRejectedValueOnce(new Error("Permission denied"));

    render(<CopyButton text="test" />);
    const btn = screen.getByRole("button");

    expect(btn.textContent).toBe("Copy");

    await act(async () => {
      fireEvent.click(btn);
    });

    expect(btn.textContent).toBe("Copy");
    expect(btn.textContent).not.toBe("Copied!");
  });
});
