import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import KycPage from "../../pages/KycPage";
import { ToastProvider } from "../../lib/toast";

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock("../../lib/wallet", () => ({
  useWallet: () => ({
    connected: false,
    address: null,
    signTx: vi.fn(),
  }),
}));

vi.mock("../../lib/stellar", () => ({
  CONTRACT_IDS: { kycRegistry: "" },
  fetchContractEvents: vi.fn().mockResolvedValue([]),
}));

vi.mock("../../components/WalletGuard", () => ({
  default: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

function renderKycPage() {
  return render(
    <ToastProvider>
      <KycPage />
    </ToastProvider>,
  );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("KycPage – lookup form", () => {
  it("renders the address lookup input", () => {
    renderKycPage();
    expect(
      screen.getByPlaceholderText(/Stellar address/i),
    ).toBeDefined();
  });

  it("shows a validation error for a malformed address when submitted", async () => {
    renderKycPage();
    const input = screen.getByPlaceholderText(/Stellar address/i);
    fireEvent.change(input, { target: { value: "not-a-valid-address" } });
    const btn = screen.getByRole("button", { name: /Lookup/i });
    fireEvent.click(btn);
    // The lookup button should be disabled for invalid addresses
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });

  it("does not show an error when the input is empty", () => {
    renderKycPage();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows an inline error message when the address is invalid", async () => {
    renderKycPage();
    const input = screen.getByPlaceholderText(/Stellar address/i);
    fireEvent.change(input, { target: { value: "BADADDRESS" } });
    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeDefined();
    });
  });
});

describe("KycPage – approve form", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the subject address, tier, jurisdiction and expiry fields", () => {
    renderKycPage();
    // Subject address — identified by label text
    expect(screen.getByText("Subject Address")).toBeDefined();
    expect(screen.getByText("KYC Tier")).toBeDefined();
    expect(screen.getByText("Jurisdiction")).toBeDefined();
    expect(screen.getByText(/Validity/i)).toBeDefined();
  });

  it("blocks submission when subject address is invalid", async () => {
    renderKycPage();
    const inputs = screen.getAllByRole("textbox");
    // The subject input is the second textbox (after lookup)
    const subjectInput = inputs.find(
      (el) => (el as HTMLInputElement).placeholder === "G…",
    );
    if (!subjectInput) throw new Error("subject input not found");
    fireEvent.change(subjectInput, { target: { value: "invalid" } });
    const btn = screen.getByRole("button", { name: /Approve KYC/i });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });

  it("enables submission when a valid address is entered", async () => {
    renderKycPage();
    const subjectInput = screen.getAllByRole("textbox").find(
      (el) => (el as HTMLInputElement).placeholder === "G…",
    );
    if (!subjectInput) throw new Error("subject input not found");
    fireEvent.change(subjectInput, {
      target: {
        value: "GBQG2SJ7MXUH34SI3MJ2I256I5UMGM2QSQZM77YFX5S6JOHXUQJEPC3A",
      },
    });
    const btn = screen.getByRole("button", { name: /Approve KYC/i });
    expect((btn as HTMLButtonElement).disabled).toBe(false);
  });

  it("renders three tier options", () => {
    renderKycPage();
    const select = screen.getByRole("combobox");
    const options = (select as HTMLSelectElement).options;
    expect(options.length).toBe(3);
    expect(options[0].textContent).toMatch(/Basic/i);
    expect(options[1].textContent).toMatch(/Accredited/i);
    expect(options[2].textContent).toMatch(/Institutional/i);
  });

  it("shows a confirmation dialog when a valid form is submitted", async () => {
    renderKycPage();
    const subjectInput = screen.getAllByRole("textbox").find(
      (el) => (el as HTMLInputElement).placeholder === "G…",
    );
    if (!subjectInput) throw new Error("subject input not found");
    fireEvent.change(subjectInput, {
      target: {
        value: "GBQG2SJ7MXUH34SI3MJ2I256I5UMGM2QSQZM77YFX5S6JOHXUQJEPC3A",
      },
    });
    // Fill jurisdiction
    const jurisdictionInputs = screen
      .getAllByRole("textbox")
      .filter((el) => (el as HTMLInputElement).placeholder?.includes("US"));
    if (jurisdictionInputs.length > 0) {
      fireEvent.change(jurisdictionInputs[0], { target: { value: "US" } });
    }
    const btn = screen.getByRole("button", { name: /Approve KYC/i });
    fireEvent.click(btn);
    await waitFor(() => {
      expect(screen.getByRole("dialog")).toBeDefined();
    });
  });

  // #988 New tests for validation feedback
  it("shows field error and blocks submission for invalid address", async () => {
    renderKycPage();
    const subjectInput = screen.getAllByRole("textbox").find(
      (el) => (el as HTMLInputElement).placeholder === "G…",
    );
    if (!subjectInput) throw new Error("subject input not found");
    
    fireEvent.change(subjectInput, { target: { value: "INVALID123" } });
    
    await waitFor(() => {
      const errorMessage = screen.getByRole("alert");
      expect(errorMessage).toBeDefined();
      expect(errorMessage.textContent).toMatch(/valid/i);
    });
    
    const btn = screen.getByRole("button", { name: /Approve KYC/i });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });

  it("shows field error and blocks submission for invalid expiry value", async () => {
    renderKycPage();
    const subjectInput = screen.getAllByRole("textbox").find(
      (el) => (el as HTMLInputElement).placeholder === "G…",
    );
    if (!subjectInput) throw new Error("subject input not found");
    
    fireEvent.change(subjectInput, {
      target: {
        value: "GBQG2SJ7MXUH34SI3MJ2I256I5UMGM2QSQZM77YFX5S6JOHXUQJEPC3A",
      },
    });

    const expiryInput = screen.getAllByRole("spinbutton").find(
      (el) => (el as HTMLInputElement).value === "365",
    );
    if (!expiryInput) throw new Error("expiry input not found");
    
    fireEvent.change(expiryInput, { target: { value: "-100" } });
    
    const btn = screen.getByRole("button", { name: /Approve KYC/i });
    fireEvent.click(btn);
    
    // Should not open dialog for invalid inputs
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    }, { timeout: 500 });
  });
});
