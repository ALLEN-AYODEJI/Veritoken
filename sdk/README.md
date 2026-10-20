# @veritoken/sdk

TypeScript SDK for the Veritoken smart contract suite on Stellar Soroban. It
wraps every contract in a typed client, resolves network configuration
(testnet / mainnet / futurenet / a local standalone node) from one place, and
gives you a typed error model and event parser instead of raw Soroban XDR.

This document is the getting-started guide and usage reference. For the full
public method / data-structure / error-code reference across all six
contracts, see [../docs/contract-api-reference.md](../docs/contract-api-reference.md).
For runnable, end-to-end examples (including this SDK vs. raw JS/Python), see
[../docs/examples/](../docs/examples/).

## Contents

- [Install](#install)
- [Getting started](#getting-started)
- [Network configuration](#network-configuration)
- [Building clients: `createClients` / `ClientFactory`](#building-clients-createclients--clientfactory)
- [Contract-by-contract reference](#contract-by-contract-reference)
- [Error handling](#error-handling)
- [Event parsing](#event-parsing)
- [Auth helpers](#auth-helpers)
- [Testing your integration](#testing-your-integration)
- [Adding a new contract client](#adding-a-new-contract-client)

## Install

The SDK lives in this monorepo as the `sdk/` workspace and isn't published to
a registry. Consume it via a workspace/path dependency, or `npm run
build:sdk` (root `package.json`) and reference `sdk/dist`.

```bash
npm install # from the repo root — installs all workspaces, including sdk
```

`@stellar/stellar-sdk` is a peer dependency — install it (`^12.0.0`) in the
consuming app.

## Getting started

The fastest path from zero to a read: pick a network, build the client(s) you
need, call a method.

```ts
import { createClients } from "@veritoken/sdk";

const { kycRegistry, rwaToken } = createClients({
  network: "testnet",
  contractIds: {
    kycRegistry: process.env.VITE_KYC_REGISTRY_ID,
    rwaToken: process.env.VITE_RWA_TOKEN_ID,
  },
});

const isApproved = await kycRegistry?.isApproved(address);
const balance = await rwaToken?.balance(address);
```

Writes take a `signTx: (xdr: string) => Promise<string>` callback — plug in
Freighter, another wallet, or a `Keypair.sign` helper for scripts:

```ts
import type { SignTx } from "@veritoken/sdk";

const signTx: SignTx = async (xdr) => {
  const { signedTxXdr } = await signTransaction(xdr, { networkPassphrase });
  return signedTxXdr;
};

await rwaToken?.transfer(fromAddress, toAddress, 100n, signTx);
```

Every write follows the same internal pipeline: build → simulate → assemble →
sign (via your `signTx`) → submit → poll for confirmation. You get back the
confirmed `GetSuccessfulTransactionResponse`, or the call throws.

## Network configuration

_Issue #394 — Multi-Network SDK Configuration Support_

The SDK ships defaults for four networks — `testnet`, `mainnet`, `futurenet`,
and `standalone` (a local quickstart node) — and resolves the network to use
from three layers, in priority order:

1. **Explicit overrides** passed to `resolveNetworkConfig` / `createClients`
2. **Environment variables** (Node `process.env`, including Vite's
   `VITE_`-prefixed equivalents so the same code works in the frontend build)
3. **Built-in defaults** for the four known networks

```ts
import { resolveNetworkConfig, createServer } from "@veritoken/sdk";

// Defaults to testnet unless VERITOKEN_NETWORK / STELLAR_NETWORK /
// VITE_STELLAR_NETWORK is set.
const config = resolveNetworkConfig();
const server = createServer(config);
```

Switch networks with no code changes by setting an env var, or by passing an
override explicitly:

```ts
// Explicit override — e.g. a script that always targets mainnet.
const server = createServer({ network: "mainnet" });

// Full control — e.g. a local standalone node with a custom RPC port.
const server = createServer({
  network: "standalone",
  rpcUrl: "http://localhost:8000/soroban/rpc",
});
```

| Field                | Overrides via `resolveNetworkConfig({...})` | Env vars consulted (first match wins) |
|-----------------------|----------------------------------------------|----------------------------------------|
| `network`             | `network`                                    | `VERITOKEN_NETWORK`, `STELLAR_NETWORK`, `VITE_STELLAR_NETWORK` |
| `rpcUrl`               | `rpcUrl`                                     | `VERITOKEN_RPC_URL`, `SOROBAN_RPC_URL`, `VITE_SOROBAN_RPC_URL` |
| `networkPassphrase`    | `networkPassphrase`                          | `VERITOKEN_NETWORK_PASSPHRASE`, `STELLAR_NETWORK_PASSPHRASE`, `VITE_STELLAR_NETWORK_PASSPHRASE` |
| `allowHttp`            | `allowHttp`                                  | `VERITOKEN_RPC_ALLOW_HTTP` (`"true"`/`"false"`) |

Validation runs on every call: an unknown network name without an explicit
`rpcUrl` + `networkPassphrase` pair, an empty `rpcUrl` / `networkPassphrase`,
or a plaintext-`http://` RPC URL on a network that doesn't expect one
(anything but `standalone`, unless `allowHttp: true` is set explicitly) all
throw `InvalidNetworkConfigError` with a message that says exactly what's
wrong — instead of failing later as an opaque RPC error.

```ts
import { isValidNetwork, KNOWN_NETWORKS } from "@veritoken/sdk";

isValidNetwork("testnet"); // true
isValidNetwork("devnet"); // false
KNOWN_NETWORKS; // ["testnet", "mainnet", "futurenet", "standalone"]
```

`createClients` / `ClientFactory` (below) accept the same `network` /
`rpcUrl` / `networkPassphrase` / `allowHttp` fields directly, so most
applications never need to call `resolveNetworkConfig` themselves.

### Custom networks and RPC endpoints (#451)

`network` isn't limited to the four built-in names — any string works, as
long as you supply both `rpcUrl` and `networkPassphrase` (there's no
built-in default to fall back to for a name the SDK doesn't recognize).
This covers a private Soroban node, a forked testnet, or per-environment CI
infrastructure without editing SDK call sites:

```ts
const server = createServer({
  network: "my-private-devnet",
  rpcUrl: "https://rpc.my-devnet.internal",
  networkPassphrase: "My Private Devnet ; 2026",
});
```

Custom networks default `allowHttp` to `false` (same safe-by-default rule
built-in networks other than `standalone` use) — pass `allowHttp: true` (or
set `VERITOKEN_RPC_ALLOW_HTTP=true`) for a plaintext local endpoint.
Env-var overrides work identically to built-in networks — set
`VERITOKEN_NETWORK=my-private-devnet` alongside `VERITOKEN_RPC_URL` and
`VERITOKEN_NETWORK_PASSPHRASE` (or the `SOROBAN_RPC_URL`/`STELLAR_NETWORK_PASSPHRASE`/
`VITE_`-prefixed equivalents) to switch entirely via environment, no code
changes required.

## Building clients: `createClients` / `ClientFactory`

_Issue #395 — Client factory and dependency-injection pattern_ (the
foundation the network config and generator below build on)

`createClients` builds every configured client from one config object,
sharing a single RPC server and network passphrase:

```ts
import { createClients } from "@veritoken/sdk";

const clients = createClients({
  network: "testnet",
  contractIds: { rwaToken: "C...", kycRegistry: "C..." },
});
// clients.rwaToken / clients.kycRegistry are set; the other four are
// `undefined` because no contract ID was given for them.
```

An entirely empty `contractIds: {}` (with no `overrides`) is accepted: it
returns `{}`, every client is `undefined`, and no RPC call is made.

`ClientFactory` wraps the same thing with typed, fail-fast accessors — useful
when a missing client should be a loud error instead of `undefined`. Its
constructor throws on an empty configuration, and `get()` throws
synchronously with an error naming the missing client:

```ts
import { ClientFactory } from "@veritoken/sdk";

const factory = new ClientFactory({ network: "testnet", contractIds: { rwaToken: "C..." } });
const rwa = factory.get("rwaToken"); // throws a clear error if not configured
factory.has("kycRegistry"); // false
```

Both accept `overrides` — the dependency-injection escape hatch for tests or
custom client subclasses:

```ts
const fakeKyc = { isApproved: async () => true } as unknown as KycRegistryClient;
const clients = createClients({
  contractIds: { kycRegistry: "C..." },
  overrides: { kycRegistry: fakeKyc },
});
```

See [docs/examples/sdk_client_factory_example.ts](../docs/examples/sdk_client_factory_example.ts)
for a complete runnable version of both patterns.

## Contract-by-contract reference

Every client extends `BaseContractClient` and takes the same constructor
shape: `new XClient(contractId, server, networkPassphrase)`. Reads simulate
and decode; writes build → simulate → sign (via your `signTx`) → submit →
confirm. Full method signatures live in the source files linked below — this
table is the map, not the territory.

| Client | Contract | Source | Highlights |
|---|---|---|---|
| `KycRegistryClient` | `kyc-registry` | [src/clients/KycRegistryClient.ts](src/clients/KycRegistryClient.ts) | `isApproved`, `getRecord`, `getTier`, `approve`/`reject`/`revoke`, verifier & admin management |
| `ComplianceEngineClient` | `compliance-engine` | [src/clients/ComplianceEngineClient.ts](src/clients/ComplianceEngineClient.ts) | `getRules`, `setRules`/`proposeRules`/`activateRules` (time-locked governance), blocklist, pause/unpause |
| `RwaTokenClient` | `rwa-token` | [src/clients/RwaTokenClient.ts](src/clients/RwaTokenClient.ts) | SEP-41 balance/transfer/allowance, `getTokenExport`, `checkKycStatus` |
| `InvoiceTokenClient` | `invoice-token` | [src/clients/InvoiceTokenClient.ts](src/clients/InvoiceTokenClient.ts) | `issue`, `settle`, `redeem`, invoice metadata |
| `PropertyTokenClient` | `property-token` | [src/clients/PropertyTokenClient.ts](src/clients/PropertyTokenClient.ts) | Fractional shares, `depositDividend`/`claimDividend`/`pendingDividend` |
| `CarbonTokenClient` | `carbon-credit-token` | [src/clients/CarbonTokenClient.ts](src/clients/CarbonTokenClient.ts) | `mint`, `retire` (returns a `RetirementReceipt`), retirement history |

```ts
import { KycRegistryClient, createServer, NETWORK_PASSPHRASES } from "@veritoken/sdk";

const server = createServer("testnet");
const kyc = new KycRegistryClient(contractId, server, NETWORK_PASSPHRASES.testnet);

await kyc.isApproved(address);
await kyc.approve(verifierAddress, subjectAddress, /* tier */ 1, /* expiry */ 0n, "US", signTx);
```

In most applications you won't construct clients directly — use
`createClients` / `ClientFactory` above instead.

## Error handling

Every contract emits numeric `#[contracterror]` codes. `errors.ts` gives you
a lookup table and two ways to turn a thrown error into something readable:

```ts
import { formatContractError, parseContractError } from "@veritoken/sdk";

try {
  await kyc.approve(verifier, subject, 1, 0n, "US", signTx);
} catch (err) {
  // "KYC tier is too low (KycTierInsufficient #5)" instead of "Error(Contract, #5)"
  console.error(formatContractError("kyc", err));
}
```

`parseContractError(contract, rawMessage)` returns the structured
`ContractError` (`{ code, name, message }`) or `null` if the message doesn't
match a known Soroban contract-error pattern. `ContractName` is one of
`"rwa" | "carbon" | "invoice" | "property" | "kyc" | "compliance"` — pass the
same short name a client was constructed with.

## Event parsing

_Issue #393 — Typed event parsing_

`parseEvents` decodes a batch of `rpc.Api.EventResponse` into `ParsedEvent`s
whose `data` is typed per event name (transfer, mint, burn, approve, freeze,
admin-transfer, KYC/compliance updates, ...). Unknown event names still
decode — `data` is just untyped.

```ts
import { parseEvents, filterByName } from "@veritoken/sdk";

const events = await server.getEvents({ ...opts });
const parsed = parseEvents(events.events);
const transfers = filterByName(parsed, "transfer"); // narrows to KnownParsedEvent<"transfer">
```

Every client also exposes a convenience `getEvents` — see
[src/clients/base.ts](src/clients/base.ts) (`GetContractEventsOptions`).

## Health and analytics

_Issue #400 — SDK health helpers_

Three clients expose a `health()` method that collects operational signals
without a wallet or write access. All reads are simulated and safe to call
from monitoring scripts or dashboards.

```ts
import { createClients } from "@veritoken/sdk";

const { complianceEngine, kycRegistry, rwaToken } = createClients({
  network: "testnet",
  contractIds: {
    complianceEngine: process.env.VITE_COMPLIANCE_ENGINE_ID,
    kycRegistry: process.env.VITE_KYC_REGISTRY_ID,
    rwaToken: process.env.VITE_RWA_TOKEN_ID,
  },
});

const ceHealth = await complianceEngine?.health();
// {
//   reachable: true,
//   latestLedger: 12345,
//   recentEventCount: 7,
//   checkedAt: "2025-01-01T00:00:00.000Z",
//   paused: false,
//   holderCount: 42
// }

const kycHealth = await kycRegistry?.health();
// { ...base, verifierCount: 3 }

const tokenHealth = await rwaToken?.health();
// { ...base, totalSupply: 1000000n }
```

The base `ContractHealth` shape (from `checkHealth` in `health.ts`) is also
exported for use in custom monitoring scripts:

```ts
import { checkHealth } from "@veritoken/sdk";

const health = await checkHealth(server, contractId, { lookbackLedgers: 500 });
if (!health.reachable) console.error("Contract unreachable:", health.error);
```

Signals collected:
- `reachable` — RPC responded without a network error
- `latestLedger` — current ledger sequence
- `recentEventCount` — events emitted in the last `lookbackLedgers` ledgers (default 1000)
- `checkedAt` — ISO timestamp of the check
- `error` — error message if `reachable` is false

Per-contract extensions: `paused` + `holderCount` (compliance), `verifierCount` (KYC), `totalSupply` (RWA token).

## Auth helpers

_Issue #397 — Local pre-flight role checks_

Admin- and verifier-gated writes (`KycRegistryClient.approve`, `addVerifier`,
...) already run a local pre-check against the on-chain roster before
building a transaction, so a wrong-caller mistake fails immediately with a
clear `AuthError` instead of a slow simulate-then-fail round trip. On-chain
`require_auth()` enforcement is unchanged either way — this only saves a
round trip. See [src/auth.ts](src/auth.ts) if you're wiring up a similar
pattern for a new gated method.

## Testing your integration

[src/testing/mockRpc.ts](src/testing/mockRpc.ts) is a reusable mock
`rpc.Server` — `simulateTransaction` / `sendTransaction` / `getTransaction` /
`getAccount` / `getEvents` — so you can exercise a client's happy paths,
contract failures, and confirmation timeouts without a live network:

```ts
import { mockServer, simSuccess, simFailure } from "@veritoken/sdk/dist/testing/mockRpc.js";
// (imported directly from source — see the note below)

const server = mockServer({ simulateByMethod: { balance: simSuccess(encodeI128(100n)) } });
const client = new RwaTokenClient(contractId, server, passphrase);
expect(await client.balance(address)).toBe(100n);
```

`mockRpc.ts` is **not** re-exported from the package's main entry point — it
imports `vi` from vitest (a devDependency), and the main entry point is the
production runtime surface. Import it via a relative/subpath path from your
own test suite instead. See any `src/clients/*.test.ts` for the established
pattern (happy path, simulation error, malformed payload, network
rejection).

## Versioning and breaking changes

_Issue #399 — SemVer strategy_

`@veritoken/sdk` follows [Semantic Versioning 2.0.0](https://semver.org). The
full policy — what counts as a breaking change, how breaking changes are
communicated, and migration guides — is in [VERSIONING.md](VERSIONING.md).

In short:
- `PATCH` — bug fixes, no API change
- `MINOR` — new exports or methods, backward-compatible
- `MAJOR` — removed/renamed exports, changed signatures, dropped peer versions

Breaking changes are announced in `CHANGELOG.md` under a `### Breaking Changes`
section and documented with a migration note in `VERSIONING.md`.

## Adding a new contract client

_Issue #392 — Contract-specific client generators_

Every client (`KycRegistryClient`, `RwaTokenClient`, ...) follows the same
convention: extend `BaseContractClient`, take
`(contractId, server, networkPassphrase)` in the constructor, and call
`this.read()` / `this.write()` for every contract method — never
`buildContractTx` / `simulateRead` / `submitContractTx` directly (those stay
internal to `base.ts` so error enrichment and the tx pipeline are applied
consistently).

Scaffold a new client instead of copy-pasting an existing one:

```bash
npm run generate:client -- --name Escrow
# or: node scripts/generate-client.mjs --name Escrow
```

This creates `src/clients/EscrowClient.ts` and `src/clients/EscrowClient.test.ts`
following the shared convention, and prints the remaining steps:

1. Add `"escrow"` to `ContractName` in [src/errors.ts](src/errors.ts) and give
   it an `ERROR_MAPS` entry (copy the shape of an existing contract's error
   list). The generated client's `super(...)` call intentionally doesn't
   type-check until this is done — that's the reminder.
2. Fill in the Read/Write API methods to match the contract's public
   functions, following the pattern of the other `*Client.ts` files.
3. Register the client in [src/factory.ts](src/factory.ts): add it to
   `ClientMap` and `CTORS`.
4. Export it from [src/index.ts](src/index.ts).
5. Replace the placeholder test with real coverage using
   [src/testing/mockRpc.ts](src/testing/mockRpc.ts) — see
   `PropertyTokenClient.test.ts` for the established pattern (read decode,
   write happy path, contract-error enrichment, malformed payload, network
   rejection).

The generator only scaffolds the two new files — it deliberately doesn't
edit `errors.ts` / `factory.ts` / `index.ts` for you. Those are small,
deliberate edits (a new error table, a new map entry, a new export) that are
easy to get wrong via text-insertion and easy to get right by hand with the
checklist above.

Run `npm run --workspace sdk generate:client -- --help` for the full CLI
reference (`--name`, `--key`, `--force`).
