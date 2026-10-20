/**
 * Vitest unit tests for the five dashboard components.
 * Issue #547
 *
 * All contract calls and the notification store are mocked — no network needed.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import type { ReactNode } from "react";

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock("../../../lib/contracts/index", () => ({
  contracts: {
    invoice: {
      balance: vi.fn(),
      getMeta: vi.fn(),
      invoiceStatus: vi.fn(),
      isSettled: vi.fn(),
      totalSupply: vi.fn(),
    },
    property: {
      balance: vi.fn(),
      getDividendHistory: vi.fn(),
      dividendDepositCount: vi.fn(),
      pendingDividend: vi.fn(),
    },
    carbon: {
      balance: vi.fn(),
    },
    kyc: {
      getRecord: vi.fn(),
    },
  },
}));

vi.mock("../../../lib/stellar", () => ({
  CONTRACT_IDS: {
    kycRegistry: "C_KYC",
    complianceEngine: "C_CE",
    invoiceToken: "C_INVOICE",
    propertyToken: "C_PROPERTY",
    carbonToken: "C_CARBON",
    rwaToken: "",
  },
}));

// recharts needs a ResizeObserver in jsdom
global.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
};

// ── Helpers ───────────────────────────────────────────────────────────────────

function wrapper({ children }: { children: ReactNode }) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return (
    <MemoryRouter>
      <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    </MemoryRouter>
  );
}

// Import contracts mock so we can cast it
import { contracts } from "../../../lib/contracts/index";
const mockContracts = contracts as unknown as {
  invoice: {
    balance: ReturnType<typeof vi.fn>;
    getMeta: ReturnType<typeof vi.fn>;
    invoiceStatus: ReturnType<typeof vi.fn>;
    isSettled: ReturnType<typeof vi.fn>;
    totalSupply: ReturnType<typeof vi.fn>;
  };
  property: {
    balance: ReturnType<typeof vi.fn>;
    getDividendHistory: ReturnType<typeof vi.fn>;
    dividendDepositCount: ReturnType<typeof vi.fn>;
    pendingDividend: ReturnType<typeof vi.fn>;
  };
  carbon: { balance: ReturnType<typeof vi.fn> };
  kyc: { getRecord: ReturnType<typeof vi.fn> };
};

// ── PortfolioSummaryCard ──────────────────────────────────────────────────────

import { PortfolioSummaryCard } from "../PortfolioSummaryCard";

describe("PortfolioSummaryCard", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("shows connect wallet placeholder when walletAddress is null", () => {
    render(<PortfolioSummaryCard walletAddress={null} />, { wrapper });
    expect(screen.getByText(/connect wallet/i)).toBeInTheDocument();
  });

  it("shows balance totals for 3 contracts with non-zero balances", async () => {
    mockContracts.invoice.balance.mockResolvedValue(500n);
    mockContracts.property.balance.mockResolvedValue(1000n);
    mockContracts.carbon.balance.mockResolvedValue(250n);

    render(
      <PortfolioSummaryCard walletAddress="GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVWX" />,
      { wrapper }
    );

    await waitFor(() => {
      expect(screen.getByText("3")).toBeInTheDocument(); // active positions
    });
    expect(screen.getByText("500")).toBeInTheDocument();
    expect(screen.getByText("1,000")).toBeInTheDocument();
    expect(screen.getByText("250")).toBeInTheDocument();
  });

  it("shows 1 active position when only one contract has a balance", async () => {
    mockContracts.invoice.balance.mockResolvedValue(100n);
    mockContracts.property.balance.mockResolvedValue(0n);
    mockContracts.carbon.balance.mockResolvedValue(0n);

    render(
      <PortfolioSummaryCard walletAddress="GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVWX" />,
      { wrapper }
    );

    await waitFor(() => {
      expect(screen.getByText("1")).toBeInTheDocument();
    });
  });
});

// ── KycExpiryPanel ────────────────────────────────────────────────────────────

import { KycExpiryPanel } from "../KycExpiryPanel";

describe("KycExpiryPanel", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("shows connect wallet placeholder when walletAddress is null", () => {
    render(<KycExpiryPanel walletAddress={null} />, { wrapper });
    expect(screen.getByText(/connect wallet/i)).toBeInTheDocument();
  });

  it("shows warning banner when expiry is within 30 days", async () => {
    const nowS = Math.floor(Date.now() / 1000);
    mockContracts.kyc.getRecord.mockResolvedValue({
      status: "Approved",
      tier: 1,
      jurisdiction: "US",
      expiry: BigInt(nowS + 5 * 86400), // 5 days from now
      verifier: "GVERIFIER",
    });

    render(
      <KycExpiryPanel walletAddress="GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVWX" />,
      { wrapper }
    );

    await waitFor(() => {
      expect(screen.getByText(/kyc expiring soon/i)).toBeInTheDocument();
    });
    expect(screen.getByText(/5 day/i)).toBeInTheDocument();
  });

  it("shows no expiry message when expiry is 0", async () => {
    mockContracts.kyc.getRecord.mockResolvedValue({
      status: "Approved",
      tier: 2,
      jurisdiction: "EU",
      expiry: 0n,
      verifier: "GVERIFIER",
    });

    render(
      <KycExpiryPanel walletAddress="GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVWX" />,
      { wrapper }
    );

    await waitFor(() => {
      expect(screen.getByText(/no expiry set/i)).toBeInTheDocument();
    });
  });

  it("shows not approved state when status is Revoked", async () => {
    mockContracts.kyc.getRecord.mockResolvedValue({
      status: "Revoked",
      tier: 0,
      jurisdiction: "NG",
      expiry: 0n,
      verifier: "GVERIFIER",
    });

    render(
      <KycExpiryPanel walletAddress="GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVWX" />,
      { wrapper }
    );

    await waitFor(() => {
      expect(screen.getByText(/not kyc approved/i)).toBeInTheDocument();
    });
    expect(screen.getByText(/revoked/i)).toBeInTheDocument();
  });
});

// ── InvoicePortfolioTable ─────────────────────────────────────────────────────

import { InvoicePortfolioTable } from "../InvoicePortfolioTable";

describe("InvoicePortfolioTable", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("shows connect wallet placeholder when walletAddress is null", () => {
    render(<InvoicePortfolioTable walletAddress={null} />, { wrapper });
    expect(screen.getByText(/connect wallet/i)).toBeInTheDocument();
  });

  it("shows no positions message when balance is 0", async () => {
    mockContracts.invoice.getMeta.mockResolvedValue({
      invoice_id: "INV-001",
      issuer: "Issuer",
      debtor: "Debtor",
      face_value_usd: 10000000n,
      discount_rate_bps: 0,
      due_date: 9999999999n,
      currency: "USD",
      ipfs_doc_hash: "",
      transfer_fee_bps: 0,
      fee_recipient: null,
      notification_webhook: "",
    });
    mockContracts.invoice.balance.mockResolvedValue(0n);
    mockContracts.invoice.invoiceStatus.mockResolvedValue(1);
    mockContracts.invoice.isSettled.mockResolvedValue(false);
    mockContracts.invoice.totalSupply.mockResolvedValue(0n);

    render(
      <InvoicePortfolioTable walletAddress="GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVWX" />,
      { wrapper }
    );

    await waitFor(() => {
      expect(screen.getByText(/no invoice positions found/i)).toBeInTheDocument();
    });
  });

  it("renders one row when wallet has a balance", async () => {
    mockContracts.invoice.getMeta.mockResolvedValue({
      invoice_id: "INV-TEST-001",
      issuer: "Issuer Co",
      debtor: "Debtor Co",
      face_value_usd: 10000000n,
      discount_rate_bps: 0,
      due_date: 9999999999n,
      currency: "USD",
      ipfs_doc_hash: "",
      transfer_fee_bps: 0,
      fee_recipient: null,
      notification_webhook: "",
    });
    mockContracts.invoice.balance.mockResolvedValue(5000000n);
    mockContracts.invoice.invoiceStatus.mockResolvedValue(1); // Issued
    mockContracts.invoice.isSettled.mockResolvedValue(false);
    mockContracts.invoice.totalSupply.mockResolvedValue(10000000n);

    render(
      <InvoicePortfolioTable walletAddress="GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVWX" />,
      { wrapper }
    );

    await waitFor(() => {
      expect(screen.getByText("INV-TEST-001")).toBeInTheDocument();
    });
    expect(screen.getByText("Issued")).toBeInTheDocument();
    // 1 data row rendered
    const rows = screen.getAllByRole("row");
    // header row + 1 data row
    expect(rows).toHaveLength(2);
  });
});

// ── ComplianceAlertTimeline ───────────────────────────────────────────────────

import { ComplianceAlertTimeline } from "../ComplianceAlertTimeline";
import { useNotificationStore } from "../../../lib/notificationStore";

describe("ComplianceAlertTimeline", () => {
  beforeEach(() => {
    useNotificationStore.getState().clearAll();
  });

  it("shows empty state when no notifications", () => {
    render(<ComplianceAlertTimeline />, { wrapper });
    expect(screen.getByText(/no alerts recorded/i)).toBeInTheDocument();
  });

  it("renders pushed notifications in reverse chronological order", () => {
    const { push } = useNotificationStore.getState();
    push({ title: "First", message: "first msg", severity: "info", category: "compliance" });
    push({ title: "Second", message: "second msg", severity: "warning", category: "compliance" });

    render(<ComplianceAlertTimeline />, { wrapper });

    const items = screen.getAllByRole("listitem");
    // Most recent (Second) should appear first
    expect(items[0]).toHaveTextContent("Second");
    expect(items[1]).toHaveTextContent("First");
  });

  it("shows at most 50 alerts", () => {
    const { push } = useNotificationStore.getState();
    for (let i = 0; i < 60; i++) {
      push({ title: `Alert ${i}`, message: "msg", severity: "info", category: "compliance" });
    }

    render(<ComplianceAlertTimeline />, { wrapper });

    const items = screen.getAllByRole("listitem");
    expect(items).toHaveLength(50);
  });
});

// ── DividendHistoryChart ──────────────────────────────────────────────────────

import { DividendHistoryChart } from "../DividendHistoryChart";

describe("DividendHistoryChart", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("shows connect wallet placeholder when walletAddress is null", () => {
    render(<DividendHistoryChart walletAddress={null} />, { wrapper });
    expect(screen.getByText(/connect wallet/i)).toBeInTheDocument();
  });

  it("shows no distributions message when count is 0", async () => {
    mockContracts.property.dividendDepositCount.mockResolvedValue(0);

    render(
      <DividendHistoryChart walletAddress="GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVWX" />,
      { wrapper }
    );

    await waitFor(() => {
      expect(screen.getByText(/no dividend distributions found/i)).toBeInTheDocument();
    });
  });

  it("renders chart container when dividend data is present", async () => {
    const now = Math.floor(Date.now() / 1000);
    mockContracts.property.dividendDepositCount.mockResolvedValue(2);
    mockContracts.property.getDividendHistory.mockResolvedValue([
      { amount: 1000n, timestamp: now - 86400, running_total_dps: 1000n, distribution_type: 0 },
      { amount: 2000n, timestamp: now - 172800, running_total_dps: 3000n, distribution_type: 0 },
    ]);
    mockContracts.property.pendingDividend.mockResolvedValue(0n);

    render(
      <DividendHistoryChart walletAddress="GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUVWX" />,
      { wrapper }
    );

    // recharts ResponsiveContainer renders a div in jsdom (no real resize);
    // assert the accessible chart wrapper is present.
    await waitFor(() => {
      expect(
        screen.getByRole("img", { name: /dividend history chart with 2 data points/i })
      ).toBeInTheDocument();
    });
  });
});

// ── DashboardPage empty state ─────────────────────────────────────────────────

import DashboardPage from "../../../pages/DashboardPage";

describe("DashboardPage empty state", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("renders without errors when all balances are zero", async () => {
    mockContracts.invoice.balance.mockResolvedValue(0n);
    mockContracts.property.balance.mockResolvedValue(0n);
    mockContracts.carbon.balance.mockResolvedValue(0n);
    mockContracts.property.dividendDepositCount.mockResolvedValue(0);
    mockContracts.kyc.getRecord.mockResolvedValue({
      status: "Pending",
      tier: 0,
      jurisdiction: "",
      expiry: 0n,
      verifier: "",
    });

    render(<DashboardPage />, { wrapper });

    await waitFor(() => {
      expect(screen.getByText("Portfolio Dashboard")).toBeInTheDocument();
    });

    expect(screen.getByText("0")).toBeInTheDocument();
    expect(screen.getByText(/no dividend distributions found/i)).toBeInTheDocument();
  });

  it("shows appropriate empty messages without misleading data", async () => {
    mockContracts.invoice.balance.mockResolvedValue(0n);
    mockContracts.property.balance.mockResolvedValue(0n);
    mockContracts.carbon.balance.mockResolvedValue(0n);
    mockContracts.invoice.getMeta.mockResolvedValue({
      invoice_id: "INV-001",
      issuer: "Issuer",
      debtor: "Debtor",
      face_value_usd: 10000000n,
      discount_rate_bps: 0,
      due_date: 9999999999n,
      currency: "USD",
      ipfs_doc_hash: "",
      transfer_fee_bps: 0,
      fee_recipient: null,
      notification_webhook: "",
    });
    mockContracts.invoice.invoiceStatus.mockResolvedValue(1);
    mockContracts.invoice.isSettled.mockResolvedValue(false);
    mockContracts.invoice.totalSupply.mockResolvedValue(0n);
    mockContracts.property.dividendDepositCount.mockResolvedValue(0);
    mockContracts.kyc.getRecord.mockResolvedValue({
      status: "Pending",
      tier: 0,
      jurisdiction: "",
      expiry: 0n,
      verifier: "",
    });

    render(<DashboardPage />, { wrapper });

    await waitFor(() => {
      expect(screen.getByText(/no invoice positions found/i)).toBeInTheDocument();
    });
    expect(screen.getByText(/no dividend distributions found/i)).toBeInTheDocument();
  });
});
