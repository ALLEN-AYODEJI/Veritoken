import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent, within } from "@testing-library/react";
import StatusPage from "../StatusPage";
import { deploymentHealth } from "../../lib/deploymentHealth";
import type { DeploymentHealthReport } from "../../lib/deploymentHealth";

vi.mock("../../lib/deploymentHealth", () => ({
  deploymentHealth: {
    fullReport: vi.fn(),
  },
}));

const healthyReport: DeploymentHealthReport = {
  healthy: true,
  overallStatus: "ok",
  entries: [
    { key: "kycRegistry", contractId: "CABC", status: "ok", message: "Contract is responsive.", latencyMs: 42, checkedAt: new Date().toISOString() },
    { key: "complianceEngine", contractId: "CDEF", status: "ok", message: "Contract is responsive.", latencyMs: 38, checkedAt: new Date().toISOString() },
  ],
  diagnostics: [],
  generatedAt: new Date().toISOString(),
};

const degradedReport: DeploymentHealthReport = {
  healthy: false,
  overallStatus: "degraded",
  entries: [
    { key: "kycRegistry", contractId: "CABC", status: "ok", message: "Contract is responsive.", latencyMs: 40, checkedAt: new Date().toISOString() },
    { key: "complianceEngine", contractId: "", status: "unreachable", message: "RPC call failed.", latencyMs: null, checkedAt: new Date().toISOString() },
  ],
  diagnostics: ["[UNREACHABLE] complianceEngine: RPC call failed."],
  generatedAt: new Date().toISOString(),
};

function renderPage() {
  return render(<StatusPage />);
}

describe("StatusPage — healthy deployment", () => {
  beforeEach(() => {
    vi.mocked(deploymentHealth.fullReport).mockResolvedValue(healthyReport);
  });

  it("renders the page heading", async () => {
    renderPage();
    expect(await screen.findByText("Deployment Status")).toBeDefined();
  });

  it("shows the Operational banner when all contracts are healthy", async () => {
    renderPage();
    // The overall status banner has role="status"; look for "Operational" within it.
    const banner = await screen.findByRole("status");
    expect(within(banner).getByText("Operational")).toBeDefined();
  });

  it("lists each contract entry", async () => {
    renderPage();
    expect(await screen.findByText("kycRegistry")).toBeDefined();
    expect(await screen.findByText("complianceEngine")).toBeDefined();
  });

  it("does not render a diagnostics section when there are none", async () => {
    renderPage();
    // Wait for the page to load before asserting absence
    await screen.findByRole("status");
    expect(screen.queryByText("Diagnostics")).toBeNull();
  });
});

describe("StatusPage — degraded deployment", () => {
  beforeEach(() => {
    vi.mocked(deploymentHealth.fullReport).mockResolvedValue(degradedReport);
  });

  it("shows the Degraded banner", async () => {
    renderPage();
    const banner = await screen.findByRole("status");
    expect(within(banner).getByText("Degraded")).toBeDefined();
  });

  it("renders the diagnostics section with the error message", async () => {
    renderPage();
    expect(await screen.findByText("Diagnostics")).toBeDefined();
    // The diagnostics card contains the full message text
    expect(await screen.findByText(/\[UNREACHABLE\] complianceEngine/i)).toBeDefined();
  });
});

describe("StatusPage — error state", () => {
  beforeEach(() => {
    vi.mocked(deploymentHealth.fullReport).mockRejectedValue(new Error("network timeout"));
  });

  it("shows an error alert when the check fails", async () => {
    renderPage();
    expect(await screen.findByText(/network timeout/i)).toBeDefined();
  });
});

describe("StatusPage — refresh", () => {
  it("calls fullReport again when Refresh now is clicked", async () => {
    vi.mocked(deploymentHealth.fullReport).mockResolvedValue(healthyReport);
    renderPage();
    await screen.findByRole("status");
    const callsBefore = vi.mocked(deploymentHealth.fullReport).mock.calls.length;
    const refreshBtn = screen.getByRole("button", { name: /refresh status/i });
    fireEvent.click(refreshBtn);
    await waitFor(() => {
      expect(vi.mocked(deploymentHealth.fullReport).mock.calls.length).toBeGreaterThan(callsBefore);
    });
  });
});

describe("StatusPage — partial endpoint failure", () => {
  beforeEach(() => {
    vi.mocked(deploymentHealth.fullReport).mockResolvedValue(degradedReport);
  });

  it("shows both healthy and failed components when one endpoint fails", async () => {
    renderPage();
    
    await waitFor(() => {
      expect(screen.getByText("kycRegistry")).toBeDefined();
      expect(screen.getByText("complianceEngine")).toBeDefined();
    });

    const banner = await screen.findByRole("status");
    expect(within(banner).getByText("Degraded")).toBeDefined();

    const okStatus = screen.getAllByText("Operational");
    expect(okStatus.length).toBeGreaterThan(0);

    expect(screen.getByText("Unreachable")).toBeDefined();
  });

  it("displays overall degraded state with individual service statuses visible", async () => {
    renderPage();

    const banner = await screen.findByRole("status");
    expect(within(banner).getByText("Degraded")).toBeDefined();

    expect(screen.getByText("kycRegistry")).toBeDefined();
    expect(screen.getByText("complianceEngine")).toBeDefined();
  });
});
