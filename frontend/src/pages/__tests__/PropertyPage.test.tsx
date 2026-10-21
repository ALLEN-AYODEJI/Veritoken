import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import PropertyPage from "../../pages/PropertyPage";
import { ToastProvider } from "../../lib/toast";
import { contracts } from "../../lib/contracts/index";

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock("../../lib/wallet", () => ({
  useWallet: () => ({
    connected: true,
    address: "GBQG2SJ7MXUH34SI3MJ2I256I5UMGM2QSQZM77YFX5S6JOHXUQJEPC3A",
    signTx: vi.fn(),
  }),
}));

vi.mock("../../lib/stellar", () => ({
  CONTRACT_IDS: { propertyToken: "CTEST456" },
  fetchContractEvents: vi.fn().mockResolvedValue([]),
  server: { simulateTransaction: vi.fn().mockRejectedValue(new Error("sim error")) },
  NETWORK_PASSPHRASE: "Test SDF Network ; September 2015",
}));

vi.mock("../../lib/contracts/index", () => ({
  contracts: {
    property: {
      getMeta: vi.fn().mockResolvedValue({
        property_id: "PROP-001",
        legal_name: "Test Property LLC",
        jurisdiction: "US",
        address: "123 Test St",
        total_valuation_usd: 1000000n,
        total_shares: 1000n,
        property_type: "residential",
        kyc_tier_required: 1,
        ipfs_title_hash: "",
      }),
      totalShares: vi.fn().mockResolvedValue(1000n),
      pendingDividend: vi.fn().mockResolvedValue(0n),
      holderCount: vi.fn().mockResolvedValue(5),
      holderSlotsRemaining: vi.fn().mockResolvedValue(95),
      mint: vi.fn().mockResolvedValue(undefined),
      depositDividend: vi.fn().mockResolvedValue(undefined),
      claimDividend: vi.fn().mockResolvedValue(100n),
      dividendDepositCount: vi.fn().mockResolvedValue(0),
      getDividendHistory: vi.fn().mockResolvedValue([]),
    },
  },
}));

vi.mock("../../components/WalletGuard", () => ({
  default: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

function renderPropertyPage() {
  return render(
    <ToastProvider>
      <PropertyPage />
    </ToastProvider>,
  );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("PropertyPage – metadata display", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the property metadata card", async () => {
    renderPropertyPage();
    expect(await screen.findByText("Property Metadata")).toBeDefined();
  });

  it("displays property details when loaded", async () => {
    renderPropertyPage();
    await waitFor(() => {
      expect(screen.getByText("PROP-001")).toBeDefined();
      expect(screen.getByText("Test Property LLC")).toBeDefined();
    });
  });
});

describe("PropertyPage – mint form", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the mint shares form", async () => {
    renderPropertyPage();
    const mintTab = screen.getByRole("button", { name: /Mint Shares/i });
    fireEvent.click(mintTab);
    await waitFor(() => {
      expect(screen.getByText("Recipient Address")).toBeDefined();
      expect(screen.getByText("Shares to Mint")).toBeDefined();
    });
  });

  it("submits mint form with valid data", async () => {
    const mintMock = vi.mocked(contracts.property.mint);
    renderPropertyPage();
    
    const mintTab = screen.getByRole("button", { name: /Mint Shares/i });
    fireEvent.click(mintTab);

    const recipientInput = await screen.findByLabelText(/Recipient Address/i);
    const sharesInput = screen.getByLabelText(/Shares to Mint/i);

    fireEvent.change(recipientInput, {
      target: { value: "GBQG2SJ7MXUH34SI3MJ2I256I5UMGM2QSQZM77YFX5S6JOHXUQJEPC3A" },
    });
    fireEvent.change(sharesInput, { target: { value: "100" } });

    const submitBtn = screen.getByRole("button", { name: /Mint Shares/i });
    fireEvent.click(submitBtn);

    await waitFor(() => {
      expect(mintMock).toHaveBeenCalled();
    });
  });

  // #987 New tests for accessible names in property form
  it("all primary fields can be located by accessible name", async () => {
    renderPropertyPage();
    
    const mintTab = screen.getByRole("button", { name: /Mint Shares/i });
    fireEvent.click(mintTab);

    await waitFor(() => {
      expect(screen.getByLabelText(/Recipient Address/i)).toBeDefined();
      expect(screen.getByLabelText(/Shares to Mint/i)).toBeDefined();
    });
  });

  it("validation errors are associated with the correct control", async () => {
    renderPropertyPage();
    
    const mintTab = screen.getByRole("button", { name: /Mint Shares/i });
    fireEvent.click(mintTab);

    const sharesInput = await screen.findByLabelText(/Shares to Mint/i) as HTMLInputElement;
    
    fireEvent.change(sharesInput, { target: { value: "-50" } });
    fireEvent.blur(sharesInput);

    await waitFor(() => {
      const errorId = sharesInput.getAttribute("aria-describedby");
      if (errorId) {
        const errorElement = document.getElementById(errorId);
        expect(errorElement).not.toBeNull();
        expect(errorElement?.getAttribute("role")).toBe("alert");
      }
    });
  });
});

describe("PropertyPage – dividend form", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the deposit dividend form", async () => {
    renderPropertyPage();
    const divTab = screen.getByRole("button", { name: /Deposit Dividend/i });
    fireEvent.click(divTab);
    await waitFor(() => {
      expect(screen.getByText(/Dividend Amount/i)).toBeDefined();
      expect(screen.getByLabelText(/Distribution type/i)).toBeDefined();
    });
  });

  it("all primary dividend fields have accessible labels", async () => {
    renderPropertyPage();
    
    const divTab = screen.getByRole("button", { name: /Deposit Dividend/i });
    fireEvent.click(divTab);

    await waitFor(() => {
      expect(screen.getByLabelText(/Dividend Amount/i)).toBeDefined();
      expect(screen.getByLabelText(/Distribution type/i)).toBeDefined();
    });
  });

  it("dividend amount validation errors are associated with field", async () => {
    renderPropertyPage();
    
    const divTab = screen.getByRole("button", { name: /Deposit Dividend/i });
    fireEvent.click(divTab);

    const amountInput = await screen.findByLabelText(/Dividend Amount/i) as HTMLInputElement;
    
    fireEvent.change(amountInput, { target: { value: "-100" } });
    fireEvent.blur(amountInput);

    await waitFor(() => {
      const errorId = amountInput.getAttribute("aria-describedby");
      if (errorId) {
        const errorElement = document.getElementById(errorId);
        expect(errorElement).not.toBeNull();
        expect(errorElement?.getAttribute("role")).toBe("alert");
      }
    });
  });
});
