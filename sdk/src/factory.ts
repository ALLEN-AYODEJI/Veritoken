/**
 * Client factory and dependency-injection pattern for the SDK (#395).
 *
 * Until now, host applications constructed each contract client directly:
 * `new RwaTokenClient(id, server, passphrase)`, repeated per contract, per
 * app. That's fine for a single script but awkward to compose (six near-
 * identical constructor calls sharing the same server/passphrase) and hard
 * to test (every consumer needs its own way to substitute a mock client).
 *
 * `createClients` builds every configured client from one config object,
 * sharing a single `rpc.Server` + network passphrase. `overrides` is the
 * escape hatch for dependency injection: pass a pre-built or mock client
 * instead of letting the factory construct one — useful for tests, or for
 * an application that wants a custom client subclass.
 *
 * @example Basic usage
 * ```ts
 * const clients = createClients({
 *   network: "testnet",
 *   contractIds: { rwaToken: "C...", kycRegistry: "C..." },
 * });
 * await clients.rwaToken?.balance(addr);
 * ```
 *
 * @example Dependency injection (testing / custom implementations)
 * ```ts
 * const fakeKyc = { isApproved: async () => true } as unknown as KycRegistryClient;
 * const clients = createClients({
 *   contractIds: { kycRegistry: "C..." },
 *   overrides: { kycRegistry: fakeKyc },
 * });
 * // clients.kycRegistry is the injected fake, never constructed for real.
 * ```
 *
 * @example ClientFactory — typed getters that throw on misconfiguration
 * ```ts
 * const factory = new ClientFactory({ network: "testnet", contractIds: { rwaToken: "C..." } });
 * const rwa = factory.get("rwaToken"); // throws a clear error if not configured
 * ```
 */

import type { rpc } from "@stellar/stellar-sdk";
import { KycRegistryClient } from "./clients/KycRegistryClient.js";
import { ComplianceEngineClient } from "./clients/ComplianceEngineClient.js";
import { InvoiceTokenClient } from "./clients/InvoiceTokenClient.js";
import { PropertyTokenClient } from "./clients/PropertyTokenClient.js";
import { CarbonTokenClient } from "./clients/CarbonTokenClient.js";
import { RwaTokenClient } from "./clients/RwaTokenClient.js";
import { createServer, resolveNetworkConfig } from "./network.js";
import type { Network } from "./types.js";

/** Every contract client the factory knows how to build, keyed by a short name. */
export interface ClientMap {
  kycRegistry: KycRegistryClient;
  complianceEngine: ComplianceEngineClient;
  invoiceToken: InvoiceTokenClient;
  propertyToken: PropertyTokenClient;
  carbonToken: CarbonTokenClient;
  rwaToken: RwaTokenClient;
}

export type ClientKey = keyof ClientMap;

type ClientCtor<K extends ClientKey> = new (
  contractId: string,
  server: rpc.Server,
  networkPassphrase: string,
) => ClientMap[K];

const CTORS: { [K in ClientKey]: ClientCtor<K> } = {
  kycRegistry: KycRegistryClient,
  complianceEngine: ComplianceEngineClient,
  invoiceToken: InvoiceTokenClient,
  propertyToken: PropertyTokenClient,
  carbonToken: CarbonTokenClient,
  rwaToken: RwaTokenClient,
};

export interface CreateClientsConfig {
  /**
   * Network to target when `server` is not injected. Falls back to
   * `VERITOKEN_NETWORK` / `STELLAR_NETWORK` / `VITE_STELLAR_NETWORK`, then
   * "testnet" — see `resolveNetworkConfig` in network.ts. @default "testnet"
   */
  network?: Network;
  /** Inject a pre-built (or mock) RPC server instead of letting the factory create one. */
  server?: rpc.Server;
  /** Overrides the passphrase implied by `network` — needed when pairing a custom `server` with a non-standard network. */
  networkPassphrase?: string;
  /** Overrides the RPC URL implied by `network`. Ignored when `server` is injected. */
  rpcUrl?: string;
  /** Allow plaintext HTTP to the RPC endpoint (e.g. a local standalone node). Ignored when `server` is injected. */
  allowHttp?: boolean;
  /** Contract IDs to build clients for. A client is only built when its ID is present. */
  contractIds: Partial<Record<ClientKey, string>>;
  /**
   * Dependency injection escape hatch: supply a pre-built or mock client
   * instead of constructing one from `contractIds`. Takes priority over
   * `contractIds` for the same key.
   */
  overrides?: Partial<ClientMap>;
}

/**
 * Build every contract client implied by `config.contractIds` (or supplied
 * via `config.overrides`), sharing one RPC server and network passphrase.
 * Keys absent from both `contractIds` and `overrides` are omitted from the
 * result. An empty `contractIds` with no `overrides` is not an error: it
 * returns `{}`, so every client is `undefined`, and makes no RPC calls. Use
 * `ClientFactory` when an empty configuration should fail instead.
 */
export function createClients(config: CreateClientsConfig): Partial<ClientMap> {
  const resolved = resolveNetworkConfig({
    network: config.network,
    rpcUrl: config.rpcUrl,
    networkPassphrase: config.networkPassphrase,
    allowHttp: config.allowHttp,
  });
  const server = config.server ?? createServer(resolved);
  const networkPassphrase = resolved.networkPassphrase;

  const clients: Partial<ClientMap> = {};

  for (const key of Object.keys(CTORS) as ClientKey[]) {
    const override = config.overrides?.[key];
    if (override) {
      const overrideId = (override as { contractId?: unknown }).contractId;
      if (typeof overrideId === "string" && overrideId.trim() === "") {
        throw new Error(`overrides.${key} has a blank contractId — supply a valid contract ID or omit the override.`);
      }
      (clients as Record<ClientKey, unknown>)[key] = override;
      continue;
    }
    const contractId = config.contractIds[key];
    if (contractId === undefined) continue;
    if (contractId.trim() === "") {
      throw new Error(`contractIds.${key} is blank — supply a valid contract ID or omit the key.`);
    }
    const Ctor = CTORS[key];
    (clients as Record<ClientKey, unknown>)[key] = new Ctor(contractId, server, networkPassphrase);
  }

  return clients;
}

/**
 * Thin wrapper around `createClients` offering typed, fail-fast accessors.
 * Prefer this over the raw `createClients` map when a missing client should
 * be a loud error rather than `undefined`.
 */
export class ClientFactory {
  private readonly clients: Partial<ClientMap>;

  constructor(config: CreateClientsConfig) {
    const { contractIds, overrides } = config ?? ({} as CreateClientsConfig);
    if (typeof contractIds !== "object" || contractIds === null || Array.isArray(contractIds)) {
      throw new Error("ClientFactory: config.contractIds must be an object keyed by client name.");
    }
    for (const [key, id] of Object.entries(contractIds)) {
      if (!(key in CTORS)) {
        throw new Error(`ClientFactory: unknown client "${key}" in contractIds.`);
      }
      if (typeof id !== "string" || id.trim() === "") {
        throw new Error(`ClientFactory: contractIds.${key} must be a non-empty contract ID string.`);
      }
    }
    const overrideCount = Object.values(overrides ?? {}).filter(Boolean).length;
    if (Object.keys(contractIds).length === 0 && overrideCount === 0) {
      throw new Error(
        "ClientFactory: no clients configured — provide at least one entry in contractIds or overrides.",
      );
    }
    this.clients = createClients(config);
  }

  /** Returns the client for `key`, or throws if it wasn't configured. */
  get<K extends ClientKey>(key: K): ClientMap[K] {
    const client = this.clients[key];
    if (!client) {
      throw new Error(
        `Client "${key}" was not configured — pass a contract ID under contractIds.${key} ` +
          `or an instance under overrides.${key}.`,
      );
    }
    return client;
  }

  /** True when `key` was configured (via contractIds or overrides). */
  has(key: ClientKey): boolean {
    return this.clients[key] !== undefined;
  }
}
