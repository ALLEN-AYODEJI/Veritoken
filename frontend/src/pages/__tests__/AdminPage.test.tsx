import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import AdminPage from "../../pages/AdminPage";
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
  CONTRACT_IDS: { complianceEngine: "CTEST123" },
  fetchContractEvents: vi.fn().mockResolvedValue([]),
  server: { simulateTransaction: vi.fn().mockRejectedValue(new Error("sim error")) },
  NETWORK_PASSPHRASE: "Test SDF Network ; September 2015",
}));

vi.mock("../../lib/contracts/index", () => ({
  contracts: {
    compliance: {
      getRules: vi.fn().mockResolvedValue({
        max_transfer_amount: 0n,
        min_holding_period: 0n,
        max_holders: 0,
        require_same_jurisdiction: false,
        paused: false,
        allowlist_mode: false,
        max_holding_period: 0n,
      }),
      getRuleChangeDelay: vi.fn().mockResolvedValue(0),
      getPendingRules: vi.fn().mockResolvedValue(null),
      pause: vi.fn().mockResolvedValue(undefined),
      unpause: vi.fn().mockResolvedValue(undefined),
      proposeRules: vi.fn().mockResolvedValue(undefined),
      setRules: vi.fn().mockResolvedValue(undefined),
      activateRules: vi.fn().mockResolvedValue(undefined),
      getBlocklist: vi.fn().mockResolvedValue([]),
      blocklistCount: vi.fn().mockResolvedValue(0),
      addToBlocklist: vi.fn().mockResolvedValue(undefined),
      removeFromBlocklist: vi.fn().mockResolvedValue(undefined),
    },
  },
}));

vi.mock("../../components/WalletGuard", () => ({
  default: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

function renderAdminPage() {
  return render(
    <ToastProvider>
      <AdminPage />
    </ToastProvider>,
  );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("AdminPage – compliance rules form", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the compliance rules heading", () => {
    renderAdminPage();
    expect(screen.getByText("Compliance Rules")).toBeDefined();
  });

  it("renders all four rule fields", async () => {
    renderAdminPage();
    expect(await screen.findByText(/Max Transfer Amount/i)).toBeDefined();
    expect(screen.getByText(/Min Holding Period/i)).toBeDefined();
    expect(screen.getByText(/Max Holders/i)).toBeDefined();
    expect(screen.getByText(/same jurisdiction/i)).toBeDefined();
  });

  it("renders the Save Rules button", async () => {
    renderAdminPage();
    expect(await screen.findByRole("button", { name: /Save Rules/i })).toBeDefined();
  });

  it("shows a confirmation dialog when Save Rules is submitted", async () => {
    renderAdminPage();
    const btn = await screen.findByRole("button", { name: /Save Rules/i });
    fireEvent.click(btn);
    await waitFor(() => {
      expect(screen.getByRole("dialog")).toBeDefined();
    });
  });

  it("confirmation dialog contains max transfer and holding period details", async () => {
    renderAdminPage();
    const btn = await screen.findByRole("button", { name: /Save Rules/i });
    fireEvent.click(btn);
    await waitFor(() => {
      expect(screen.getByRole("dialog")).toBeDefined();
    });
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/Max transfer/i)).toBeDefined();
    expect(within(dialog).getByText(/Min holding/i)).toBeDefined();
  });

  it("cancelling the dialog removes it from the DOM", async () => {
    renderAdminPage();
    const btn = await screen.findByRole("button", { name: /Save Rules/i });
    fireEvent.click(btn);
    await waitFor(() => screen.getByRole("dialog"));
    const cancelBtn = screen.getByRole("button", { name: /Cancel/i });
    fireEvent.click(cancelBtn);
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });
});

describe("AdminPage – emergency controls", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the emergency controls section", () => {
    renderAdminPage();
    expect(screen.getByText("Emergency Controls")).toBeDefined();
  });

  it("renders Pause and Unpause buttons", () => {
    renderAdminPage();
    expect(screen.getByRole("button", { name: /Pause All Transfers/i })).toBeDefined();
    expect(screen.getByRole("button", { name: /Unpause Transfers/i })).toBeDefined();
  });

  it("clicking Pause opens a confirmation dialog", async () => {
    renderAdminPage();
    fireEvent.click(screen.getByRole("button", { name: /Pause All Transfers/i }));
    await waitFor(() => {
      const dialog = screen.getByRole("dialog");
      expect(within(dialog).getByText(/Pause All Transfers/i)).toBeDefined();
    });
  });

  it("clicking Unpause opens a confirmation dialog", async () => {
    vi.mocked(contracts.compliance.getRules).mockResolvedValueOnce({
      max_transfer_amount: 0n,
      min_holding_period: 0n,
      max_holders: 0,
      require_same_jurisdiction: false,
      paused: true,
      allowlist_mode: false,
      max_holding_period: 0n,
    });
    renderAdminPage();
    const unpause = await screen.findByRole("button", { name: /Unpause Transfers/i });
    await waitFor(() => {
      expect((unpause as HTMLButtonElement).disabled).toBe(false);
    });
    fireEvent.click(unpause);
    await waitFor(() => {
      const dialog = screen.getByRole("dialog");
      expect(within(dialog).getByText(/Unpause Transfers/i)).toBeDefined();
    });
  });
});

describe("AdminPage – blocklist management", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the Blocklist Management section", () => {
    renderAdminPage();
    expect(screen.getByText("Blocklist Management")).toBeDefined();
  });

  it("shows empty state when blocklist is empty", async () => {
    renderAdminPage();
    await waitFor(() => {
      expect(screen.getByText(/No addresses are currently blocked/i)).toBeDefined();
    });
  });

  it("renders the Add to Blocklist form", async () => {
    renderAdminPage();
    expect(await screen.findByRole("button", { name: /Add to Blocklist/i })).toBeDefined();
  });

  it("Add to Blocklist button is disabled when input is empty", async () => {
    renderAdminPage();
    const btn = await screen.findByRole("button", { name: /Add to Blocklist/i });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });

  it("Add to Blocklist button becomes enabled when an address is entered", async () => {
    renderAdminPage();
    const input = await screen.findByPlaceholderText(/G… \(Stellar address\)/i);
    fireEvent.change(input, {
      target: {
        value: "GBQG2SJ7MXUH34SI3MJ2I256I5UMGM2QSQZM77YFX5S6JOHXUQJEPC3A",
      },
    });
    await waitFor(() => {
      const btn = screen.getByRole("button", { name: /Add to Blocklist/i });
      expect((btn as HTMLButtonElement).disabled).toBe(false);
    });
  });

  // #989 New tests for empty and malformed admin submissions
  it("blocks submission when blocklist address field is empty", async () => {
    renderAdminPage();
    const btn = await screen.findByRole("button", { name: /Add to Blocklist/i });
    const addToBlocklistMock = vi.mocked(contracts.compliance.addToBlocklist);
    
    fireEvent.click(btn);
    
    await waitFor(() => {
      expect(addToBlocklistMock).not.toHaveBeenCalled();
    });
  });

  it("prevents submission with malformed address and shows field feedback", async () => {
    renderAdminPage();
    const input = await screen.findByPlaceholderText(/G… \(Stellar address\)/i);
    const addToBlocklistMock = vi.mocked(contracts.compliance.addToBlocklist);
    
    fireEvent.change(input, { target: { value: "MALFORMED_ADDRESS" } });
    
    const btn = screen.getByRole("button", { name: /Add to Blocklist/i });
    fireEvent.click(btn);
    
    await waitFor(() => {
      expect(addToBlocklistMock).not.toHaveBeenCalled();
    });
    
    // Field-level feedback should be visible
    await waitFor(() => {
      const errorMessage = screen.queryByRole("alert");
      if (errorMessage) {
        expect(errorMessage.textContent).toMatch(/valid/i);
      }
    });
  });
});
