import { describe, it, expect } from "vitest";
import { Networks } from "@stellar/stellar-sdk";
import { createClients, ClientFactory, type ClientKey } from "./factory.js";
import { KycRegistryClient } from "./clients/KycRegistryClient.js";
import { RwaTokenClient } from "./clients/RwaTokenClient.js";
import { mockServer } from "./testing/mockRpc.js";

const KYC_ID = "CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA";
const RWA_ID = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";

const ALL_CLIENT_KEYS: readonly ClientKey[] = [
  "kycRegistry",
  "complianceEngine",
  "invoiceToken",
  "propertyToken",
  "carbonToken",
  "rwaToken",
];

describe("createClients", () => {
  it("builds only the clients whose contract ID is present", () => {
    const server = mockServer();
    const clients = createClients({ server, contractIds: { kycRegistry: KYC_ID } });
    expect(clients.kycRegistry).toBeInstanceOf(KycRegistryClient);
    expect(clients.rwaToken).toBeUndefined();
  });

  it("builds multiple clients sharing the same injected server", () => {
    const server = mockServer();
    const clients = createClients({
      server,
      contractIds: { kycRegistry: KYC_ID, rwaToken: RWA_ID },
    });
    expect(clients.kycRegistry).toBeInstanceOf(KycRegistryClient);
    expect(clients.rwaToken).toBeInstanceOf(RwaTokenClient);
  });

  it("defaults to a real testnet server when none is injected", () => {
    const clients = createClients({ contractIds: { kycRegistry: KYC_ID } });
    expect(clients.kycRegistry).toBeInstanceOf(KycRegistryClient);
  });

  it("dependency injection: overrides take priority over contractIds for the same key", () => {
    const fakeKyc = { isApproved: async () => true } as unknown as KycRegistryClient;
    const clients = createClients({
      server: mockServer(),
      contractIds: { kycRegistry: KYC_ID },
      overrides: { kycRegistry: fakeKyc },
    });
    expect(clients.kycRegistry).toBe(fakeKyc);
  });

  it("dependency injection: a mock client can be supplied with no contract ID at all", () => {
    const fakeRwa = { balance: async () => 42n } as unknown as RwaTokenClient;
    const clients = createClients({ contractIds: {}, overrides: { rwaToken: fakeRwa } });
    expect(clients.rwaToken).toBe(fakeRwa);
  });

  it("rejects blank contract IDs in contractIds and overrides", () => {
    const server = mockServer();
    expect(() => createClients({ server, contractIds: { kycRegistry: "   " } })).toThrow(
      /contractIds\.kycRegistry is blank/,
    );
    const blankOverride = { contractId: "" } as unknown as RwaTokenClient;
    expect(() => createClients({ server, contractIds: {}, overrides: { rwaToken: blankOverride } })).toThrow(
      /overrides\.rwaToken has a blank contractId/,
    );
  });

  it("respects an explicit networkPassphrase override", () => {
    const server = mockServer();
    const clients = createClients({
      server,
      networkPassphrase: Networks.PUBLIC,
      contractIds: { kycRegistry: KYC_ID },
    });
    expect(clients.kycRegistry).toBeInstanceOf(KycRegistryClient);
  });
});

describe("ClientFactory", () => {
  it("get() returns a configured client", () => {
    const factory = new ClientFactory({ server: mockServer(), contractIds: { kycRegistry: KYC_ID } });
    expect(factory.get("kycRegistry")).toBeInstanceOf(KycRegistryClient);
  });

  it("get() throws a descriptive error for an unconfigured client", () => {
    const factory = new ClientFactory({ server: mockServer(), contractIds: { kycRegistry: KYC_ID } });
    expect(() => factory.get("rwaToken")).toThrow(/rwaToken.*not configured/s);
  });

  it("has() reflects configuration state", () => {
    const factory = new ClientFactory({ server: mockServer(), contractIds: { kycRegistry: KYC_ID } });
    expect(factory.has("kycRegistry")).toBe(true);
    expect(factory.has("rwaToken")).toBe(false);
  });

  it("constructor rejects an empty or malformed config up front", () => {
    const server = mockServer();
    expect(() => new ClientFactory({ server, contractIds: {} })).toThrow(/no clients configured/);
    expect(() => new ClientFactory({ server, contractIds: [] as unknown as {} })).toThrow(/must be an object/);
    expect(() => new ClientFactory({ server, contractIds: { kycRegistry: "  " } })).toThrow(
      /contractIds\.kycRegistry must be a non-empty/,
    );
    expect(
      () => new ClientFactory({ server, contractIds: { bogus: KYC_ID } as unknown as { kycRegistry: string } }),
    ).toThrow(/unknown client "bogus"/);
  });

  it("get() returns an injected override", () => {
    const fakeKyc = {} as KycRegistryClient;
    const factory = new ClientFactory({ contractIds: {}, overrides: { kycRegistry: fakeKyc } });
    expect(factory.get("kycRegistry")).toBe(fakeKyc);
  });
});

// ── Empty contract ID map — #971 ──────────────────────────────────────────────

describe("createClients with no configured contract IDs", () => {
  it("returns an empty map in which every client is undefined", () => {
    const clients = createClients({ server: mockServer(), contractIds: {} });
    expect(clients).toEqual({});
    expect(Object.keys(clients)).toHaveLength(0);
    for (const key of ALL_CLIENT_KEYS) {
      expect(clients[key]).toBeUndefined();
    }
  });

  it("treats an empty overrides object the same as no overrides", () => {
    const clients = createClients({ server: mockServer(), contractIds: {}, overrides: {} });
    expect(clients).toEqual({});
  });

  it("makes no RPC calls while building the empty map", () => {
    const server = mockServer();
    createClients({ server, contractIds: {} });
    for (const method of Object.values(server as unknown as Record<string, unknown>)) {
      expect(method).not.toHaveBeenCalled();
    }
  });

  it("contrasts with ClientFactory, which rejects the same empty configuration up front", () => {
    const server = mockServer();
    expect(() => new ClientFactory({ server, contractIds: {} })).toThrow(/no clients configured/);
    expect(() => new ClientFactory({ server, contractIds: {}, overrides: {} })).toThrow(
      /no clients configured/,
    );
  });
});

// ── Fail-fast get() for a known but unconfigured client — #972 ────────────────

describe("ClientFactory.get for a known but unconfigured client", () => {
  const makeFactory = () =>
    new ClientFactory({ server: mockServer(), contractIds: { kycRegistry: KYC_ID } });

  it("throws synchronously with an Error naming the missing client", () => {
    const factory = makeFactory();
    let result: unknown;
    let caught: unknown;
    try {
      result = factory.get("rwaToken");
    } catch (err) {
      caught = err;
    }
    // A thrown error (not a returned rejected promise) proves the failure is synchronous.
    expect(result).toBeUndefined();
    expect(caught).toBeInstanceOf(Error);
    const message = (caught as Error).message;
    expect(message).toContain("rwaToken");
    expect(message).toMatch(/not configured/i);
    expect(message).not.toContain("kycRegistry");
  });

  it("names whichever client is missing", () => {
    const factory = makeFactory();
    for (const key of ALL_CLIENT_KEYS.filter((k) => k !== "kycRegistry")) {
      expect(() => factory.get(key)).toThrow(key);
    }
  });

  it("keeps configured clients retrievable after a failed lookup", () => {
    const factory = makeFactory();
    expect(() => factory.get("rwaToken")).toThrow(Error);
    expect(factory.get("kycRegistry")).toBeInstanceOf(KycRegistryClient);
    expect(factory.has("kycRegistry")).toBe(true);
    expect(factory.has("rwaToken")).toBe(false);
  });
});
