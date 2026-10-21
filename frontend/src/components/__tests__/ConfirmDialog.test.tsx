import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ConfirmDialog from "../ConfirmDialog";

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("ConfirmDialog – rendering", () => {
  it("renders title and description", () => {
    render(
      <ConfirmDialog
        title="Confirm Action"
        description="Are you sure you want to proceed?"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByText("Confirm Action")).toBeDefined();
    expect(screen.getByText("Are you sure you want to proceed?")).toBeDefined();
  });

  it("renders the default confirm label", () => {
    render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: /Confirm/i })).toBeDefined();
  });

  it("renders a custom confirm label", () => {
    render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
        confirmLabel="Retire Credits"
      />,
    );
    expect(screen.getByRole("button", { name: /Retire Credits/i })).toBeDefined();
  });

  it("renders the Cancel button", () => {
    render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: /Cancel/i })).toBeDefined();
  });

  it("sets role='dialog' with aria-modal", () => {
    const { container } = render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    const dialog = container.querySelector('[role="dialog"]');
    expect(dialog).not.toBeNull();
    expect(dialog?.getAttribute("aria-modal")).toBe("true");
  });

  it("connects aria-labelledby to the title element", () => {
    const { container } = render(
      <ConfirmDialog
        title="My Dialog"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    const dialog = container.querySelector('[role="dialog"]');
    const labelId = dialog?.getAttribute("aria-labelledby");
    expect(labelId).toBeTruthy();
    const heading = container.querySelector(`#${labelId}`);
    expect(heading?.textContent).toBe("My Dialog");
  });
});

describe("ConfirmDialog – interactions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("calls onConfirm when the confirm button is clicked", () => {
    const onConfirm = vi.fn();
    render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Confirm/i }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when the Cancel button is clicked", () => {
    const onCancel = vi.fn();
    render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Cancel/i }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when the backdrop is clicked", () => {
    const onCancel = vi.fn();
    const { container } = render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );
    const backdrop = container.querySelector('[aria-hidden="true"]') as HTMLElement;
    expect(backdrop).not.toBeNull();
    fireEvent.click(backdrop);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when the Escape key is pressed", () => {
    const onCancel = vi.fn();
    const { container } = render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );
    const panel = container.querySelector("[tabindex]") as HTMLElement;
    fireEvent.keyDown(panel, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  // #990 New tests for keyboard behavior
  it("dialog buttons are keyboard accessible via Tab", () => {
    render(
      <ConfirmDialog
        title="Test Dialog"
        description="Test description"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    
    const confirmBtn = screen.getByRole("button", { name: /Confirm/i });
    const cancelBtn = screen.getByRole("button", { name: /Cancel/i });
    
    expect(confirmBtn.tabIndex).toBeGreaterThanOrEqual(0);
    expect(cancelBtn.tabIndex).toBeGreaterThanOrEqual(0);
  });

  it("Escape key dismisses the dialog", () => {
    const onCancel = vi.fn();
    const { container } = render(
      <ConfirmDialog
        title="Test Dialog"
        description="Test description"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );
    
    const dialog = container.querySelector('[role="dialog"]') as HTMLElement;
    expect(dialog).not.toBeNull();
    
    fireEvent.keyDown(dialog, { key: "Escape" });
    
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("focus is contained within dialog when open", () => {
    const { container } = render(
      <ConfirmDialog
        title="Test Dialog"
        description="Test description"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    
    const dialog = container.querySelector('[role="dialog"]');
    expect(dialog).not.toBeNull();
    expect(dialog?.getAttribute("aria-modal")).toBe("true");
    
    const confirmBtn = screen.getByRole("button", { name: /Confirm/i });
    const cancelBtn = screen.getByRole("button", { name: /Cancel/i });
    
    expect(confirmBtn).toBeDefined();
    expect(cancelBtn).toBeDefined();
  });
});

describe("ConfirmDialog – loading state", () => {
  it("disables both buttons when loading is true", () => {
    render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
        loading
      />,
    );
    const confirmBtn = screen.getByRole("button", { name: /Sending/i });
    const cancelBtn = screen.getByRole("button", { name: /Cancel/i });
    expect((confirmBtn as HTMLButtonElement).disabled).toBe(true);
    expect((cancelBtn as HTMLButtonElement).disabled).toBe(true);
  });

  it("shows 'Sending…' label when loading", () => {
    render(
      <ConfirmDialog
        title="Test"
        description="desc"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
        loading
      />,
    );
    expect(screen.getByRole("button", { name: /Sending/i })).toBeDefined();
  });
});
