import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { EventFeed } from "../EventFeed";
import type { ContractEvent } from "../../types";

const mockEvents: ContractEvent[] = [
  {
    id: "ev1",
    type: "transfer",
    amount: "1000",
    counterparty: "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNA",
    timestamp: "2026-07-22T17:00:00Z",
  },
  {
    id: "ev2",
    type: "mint",
    amount: "500",
    counterparty: "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNB",
    timestamp: "2026-07-22T16:00:00Z",
  },
];

beforeEach(() => {
  Object.assign(navigator, {
    clipboard: {
      writeText: vi.fn().mockResolvedValue(undefined),
    },
  });
});

describe("EventFeed", () => {
  it("shows skeleton loading state", () => {
    const { container } = render(<EventFeed events={[]} loading={true} />);
    const skeletonElements = container.querySelectorAll("[style*='animation']");
    expect(skeletonElements.length).toBeGreaterThan(0);
  });

  it("shows empty state when there are no events", () => {
    render(<EventFeed events={[]} loading={false} />);
    expect(screen.getByText("No recent events found.")).toBeDefined();
  });

  it("renders events when provided", () => {
    render(<EventFeed events={mockEvents} loading={false} />);
    expect(screen.getByText("transfer")).toBeDefined();
    expect(screen.getByText("mint")).toBeDefined();
    expect(screen.getByText("1000")).toBeDefined();
    expect(screen.getByText("500")).toBeDefined();
  });

  it("renders all event rows", () => {
    const { container } = render(<EventFeed events={mockEvents} loading={false} />);
    const rows = container.querySelectorAll("tbody tr");
    expect(rows.length).toBe(2);
  });

  it("displays custom title", () => {
    render(<EventFeed events={[]} loading={false} title="Custom Feed Title" />);
    expect(screen.getByText("Custom Feed Title")).toBeDefined();
  });

  it("renders refresh button when onRefresh is provided", () => {
    const onRefresh = vi.fn();
    render(<EventFeed events={[]} loading={false} onRefresh={onRefresh} />);
    const refreshBtn = screen.getByRole("button", { name: "Refresh events" });
    expect(refreshBtn).toBeDefined();
  });

  it("calls onRefresh when refresh button is clicked", () => {
    const onRefresh = vi.fn();
    render(<EventFeed events={mockEvents} loading={false} onRefresh={onRefresh} />);
    const refreshBtn = screen.getByRole("button", { name: "Refresh events" });
    fireEvent.click(refreshBtn);
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("does not render refresh button when onRefresh is not provided", () => {
    render(<EventFeed events={[]} loading={false} />);
    expect(screen.queryByRole("button", { name: "Refresh events" })).toBeNull();
  });

  it("renders copy buttons for each event counterparty", () => {
    render(<EventFeed events={mockEvents} loading={false} />);
    const copyButtons = screen.getAllByRole("button", { name: /Copy counterparty address/i });
    expect(copyButtons.length).toBe(2);
  });

  it("handles empty events array gracefully", () => {
    render(<EventFeed events={[]} loading={false} />);
    expect(screen.getByText("No recent events found.")).toBeDefined();
  });

  it("shows clear non-error empty state message", () => {
    render(<EventFeed events={[]} loading={false} />);
    
    const emptyState = screen.getByRole("status");
    expect(emptyState).toBeDefined();
    
    expect(screen.getByText("No recent events found.")).toBeDefined();
    expect(screen.getByText("Events will appear here once contract activity occurs.")).toBeDefined();
  });

  it("does not render event rows when events array is empty", () => {
    const { container } = render(<EventFeed events={[]} loading={false} />);
    const rows = container.querySelectorAll("tbody tr");
    expect(rows.length).toBe(0);
  });

  it("empty state passes with normal rendering tests", () => {
    render(<EventFeed events={[]} loading={false} />);
    expect(screen.getByText("No recent events found.")).toBeDefined();

    render(<EventFeed events={mockEvents} loading={false} />);
    expect(screen.getByText("transfer")).toBeDefined();
    expect(screen.getByText("mint")).toBeDefined();
  });
});
