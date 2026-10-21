#!/usr/bin/env tsx
/**
 * deploy.ts — Veritoken contract deployment script
 *
 * Reads a deployment config JSON, uploads WASMs to the Stellar network,
 * deploys contracts that have changed (by comparing WASM hashes against an
 * existing manifest), and writes a manifest JSON with new contract IDs and
 * WASM hashes.
 *
 * Environment variables:
 *   DEPLOYER_SECRET           — Stellar secret key of the deployer account
 *   STELLAR_RPC_URL           — Soroban RPC endpoint
 *   STELLAR_NETWORK_PASSPHRASE — Network passphrase
 *   WASM_DIR                  — Directory containing compiled *.wasm files
 *   CONFIG_FILE               — Path to deployment config JSON
 *   MANIFEST_OUT              — Output path for the deployment manifest JSON
 *
 * The script can also be run locally against a Docker standalone node:
 *   DEPLOYER_SECRET=S... STELLAR_RPC_URL=http://localhost:8000/soroban/rpc \
 *   STELLAR_NETWORK_PASSPHRASE="Standalone Network ; February 2017" \
 *   WASM_DIR=../../target/wasm32v1-none/release \
 *   CONFIG_FILE=../config.testnet.json \
 *   MANIFEST_OUT=./manifests/local-dev.json \
 *   npx tsx deploy.ts
 */

import * as fs from "node:fs";
import * as path from "node:path";
import * as crypto from "node:crypto";
import { pathToFileURL } from "node:url";
import {
  Keypair,
  SorobanRpc,
  TransactionBuilder,
  Networks,
  BASE_FEE,
  xdr,
  Address,
  nativeToScVal,
} from "@stellar/stellar-sdk";

// ── Configuration ─────────────────────────────────────────────────────────────

// Populated by loadEnv() when the script runs, so the exported helpers can be
// imported by tests without deployment credentials in the environment.
let DEPLOYER_SECRET: string;
let RPC_URL: string;
let NETWORK_PASSPHRASE: string;
let WASM_DIR: string;
let CONFIG_FILE: string;
let MANIFEST_OUT: string;

function loadEnv(): void {
  DEPLOYER_SECRET = requireEnv("DEPLOYER_SECRET");
  RPC_URL = requireEnv("STELLAR_RPC_URL");
  NETWORK_PASSPHRASE =
    process.env.STELLAR_NETWORK_PASSPHRASE ?? Networks.TESTNET;
  WASM_DIR = requireEnv("WASM_DIR");
  CONFIG_FILE = requireEnv("CONFIG_FILE");
  MANIFEST_OUT = requireEnv("MANIFEST_OUT");
}

function requireEnv(key: string): string {
  const value = process.env[key];
  if (!value) {
    throw new Error(`Required environment variable ${key} is not set.`);
  }
  return value;
}

// ── Types ─────────────────────────────────────────────────────────────────────

interface ContractConfig {
  name: string;
  enabled?: boolean;
  artifact: string;
  env_key: string;
  dependencies: string[];
  deployment_mode: "initialize" | "constructor";
  initialize_function?: string;
  initialize_args?: Record<string, unknown>;
  constructor_args?: Record<string, unknown>;
  declared_metadata?: Record<string, unknown>;
  health_check?: {
    function: string;
    args?: Record<string, unknown>;
    expectation: string;
  };
}

interface DeployConfig {
  schema_version: number;
  profile: string;
  contracts: ContractConfig[];
}

interface ManifestEntry {
  contract_id: string;
  wasm_hash: string;
  deployed_at: string;
  network: string;
}

interface Manifest {
  schema_version: number;
  git_sha: string;
  network: string;
  deployed_at: string;
  contracts: Record<string, ManifestEntry>;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function sha256Hex(data: Buffer): string {
  return crypto.createHash("sha256").update(data).digest("hex");
}

/**
 * Parse a comma-separated string of contract identifiers.
 *
 * Each entry is trimmed; empty entries (produced by trailing commas, double
 * commas, or whitespace-only segments) are rejected by throwing an error.
 * This prevents ambiguous manifests and upstream deployment failures that are
 * hard to diagnose.
 *
 * @param raw - A comma-separated string of contract IDs, e.g. "kyc-registry,rwa-token".
 * @returns   - An array of non-empty, trimmed contract ID strings.
 * @throws    - If any entry is empty after trimming.
 */
export function parseContractIds(raw: string): string[] {
  const entries = raw.split(",");
  const result: string[] = [];
  for (const entry of entries) {
    const trimmed = entry.trim();
    if (trimmed === "") {
      throw new Error(
        `parseContractIds: empty contract identifier in input "${raw}". ` +
          "Remove trailing commas and blank entries before proceeding."
      );
    }
    result.push(trimmed);
  }
  return result;
}

/**
 * Parse a deployment config JSON string and validate contract entries.
 *
 * Every contract must have a non-empty `name`; blank names would otherwise be
 * written into the deployment manifest as ambiguous keys.
 *
 * @param raw - The raw deployment config JSON.
 * @returns   - The parsed deployment config.
 * @throws    - If any contract name is missing, not a string, or blank.
 */
export function loadConfig(raw: string): DeployConfig {
  const config: DeployConfig = JSON.parse(raw);
  (config.contracts ?? []).forEach((contract, index) => {
    if (typeof contract.name !== "string" || contract.name.trim() === "") {
      throw new Error(
        `loadConfig: contract at index ${index} must have a non-empty name.`
      );
    }
  });
  return config;
}

function resolveArtifactPath(artifactTemplate: string): string {
  const resolved = artifactTemplate.replace("${wasm_dir}", WASM_DIR);
  if (!fs.existsSync(resolved)) {
    throw new Error(`WASM artifact not found: ${resolved}`);
  }
  return resolved;
}

function interpolateArgs(
  args: Record<string, unknown>,
  resolvedIds: Record<string, string>
): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    if (typeof value === "string") {
      let resolved = value;
      // ${admin} → deployer public key
      if (resolved === "${admin}") {
        resolved = Keypair.fromSecret(DEPLOYER_SECRET).publicKey();
      }
      // ${contract.name} → already-deployed contract ID
      const contractRef = resolved.match(/^\$\{contract\.(\w+)\}$/);
      if (contractRef) {
        const depName = contractRef[1];
        if (!resolvedIds[depName]) {
          throw new Error(
            `Dependency contract '${depName}' has not been deployed yet.`
          );
        }
        resolved = resolvedIds[depName];
      }
      result[key] = resolved;
    } else if (value !== null && typeof value === "object") {
      result[key] = interpolateArgs(
        value as Record<string, unknown>,
        resolvedIds
      );
    } else {
      result[key] = value;
    }
  }
  return result;
}

async function waitForTransaction(
  server: SorobanRpc.Server,
  hash: string
): Promise<SorobanRpc.Api.GetTransactionResponse> {
  const MAX_ATTEMPTS = 30;
  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    const response = await server.getTransaction(hash);
    if (
      response.status !== SorobanRpc.Api.GetTransactionStatus.NOT_FOUND
    ) {
      return response;
    }
    await new Promise((resolve) => setTimeout(resolve, 2000));
  }
  throw new Error(`Transaction ${hash} not confirmed after ${MAX_ATTEMPTS} attempts.`);
}

// ── Upload WASM ───────────────────────────────────────────────────────────────

async function uploadWasm(
  server: SorobanRpc.Server,
  keypair: Keypair,
  wasmBuffer: Buffer
): Promise<string> {
  const account = await server.getAccount(keypair.publicKey());

  const uploadOp = xdr.Operation.fromXDR(
    new xdr.Operation({
      sourceAccount: null,
      body: xdr.OperationBody.uploadContractWasm({
        wasm: wasmBuffer,
      }),
    }).toXDR(),
    "raw"
  );

  let tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(uploadOp)
    .setTimeout(30)
    .build();

  tx = await server.prepareTransaction(tx);
  tx.sign(keypair);

  const sendResult = await server.sendTransaction(tx);
  if (sendResult.status === "ERROR") {
    throw new Error(`Upload failed: ${JSON.stringify(sendResult.errorResult)}`);
  }

  const result = await waitForTransaction(server, sendResult.hash);
  if (result.status !== SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error(`WASM upload transaction failed: ${result.status}`);
  }

  // The WASM hash is the SHA-256 of the uploaded bytes
  return sha256Hex(wasmBuffer);
}

// ── Deploy Contract ───────────────────────────────────────────────────────────

async function deployContract(
  server: SorobanRpc.Server,
  keypair: Keypair,
  wasmHash: string,
  contractConfig: ContractConfig,
  resolvedIds: Record<string, string>
): Promise<string> {
  const account = await server.getAccount(keypair.publicKey());

  const wasmHashBytes = Buffer.from(wasmHash, "hex");

  const createOp = xdr.Operation.fromXDR(
    new xdr.Operation({
      sourceAccount: null,
      body: xdr.OperationBody.invokeHostFunction({
        hostFunction: xdr.HostFunction.hostFunctionTypeCreateContract(
          new xdr.CreateContractArgs({
            contractIdPreimage:
              xdr.ContractIdPreimage.contractIdPreimageFromAddress(
                new xdr.ContractIdPreimageFromAddress({
                  address: Address.fromString(
                    keypair.publicKey()
                  ).toScAddress(),
                  salt: Buffer.from(crypto.randomBytes(32)),
                })
              ),
            executable: xdr.ContractExecutable.contractExecutableWasm(
              wasmHashBytes
            ),
          })
        ),
        auth: [],
      }),
    }).toXDR(),
    "raw"
  );

  let tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(createOp)
    .setTimeout(30)
    .build();

  tx = await server.prepareTransaction(tx);
  tx.sign(keypair);

  const sendResult = await server.sendTransaction(tx);
  if (sendResult.status === "ERROR") {
    throw new Error(
      `Contract creation failed: ${JSON.stringify(sendResult.errorResult)}`
    );
  }

  const result = await waitForTransaction(server, sendResult.hash);
  if (result.status !== SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error(`Contract creation transaction failed: ${result.status}`);
  }

  // Extract the new contract ID from the return value
  if (!result.returnValue) {
    throw new Error("Contract creation returned no value.");
  }
  const contractId = Address.fromScVal(result.returnValue).toString();
  return contractId;
}

// ── Initialize Contract ───────────────────────────────────────────────────────

async function initializeContract(
  server: SorobanRpc.Server,
  keypair: Keypair,
  contractId: string,
  functionName: string,
  args: Record<string, unknown>
): Promise<void> {
  const account = await server.getAccount(keypair.publicKey());

  const scArgs = Object.values(args).map((v) => nativeToScVal(v));

  const invokeOp = xdr.Operation.fromXDR(
    new xdr.Operation({
      sourceAccount: null,
      body: xdr.OperationBody.invokeHostFunction({
        hostFunction: xdr.HostFunction.hostFunctionTypeInvokeContract(
          new xdr.InvokeContractArgs({
            contractAddress: Address.fromString(contractId).toScAddress(),
            functionName,
            args: scArgs,
          })
        ),
        auth: [],
      }),
    }).toXDR(),
    "raw"
  );

  let tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(invokeOp)
    .setTimeout(30)
    .build();

  tx = await server.prepareTransaction(tx);
  tx.sign(keypair);

  const sendResult = await server.sendTransaction(tx);
  if (sendResult.status === "ERROR") {
    throw new Error(
      `Initialization failed: ${JSON.stringify(sendResult.errorResult)}`
    );
  }

  const result = await waitForTransaction(server, sendResult.hash);
  if (result.status !== SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error(`Initialize transaction failed: ${result.status}`);
  }
}

// ── Main ──────────────────────────────────────────────────────────────────────

async function main(): Promise<void> {
  loadEnv();
  console.log("=== Veritoken Deployment Script ===");
  console.log(`RPC URL:        ${RPC_URL}`);
  console.log(`Config:         ${CONFIG_FILE}`);
  console.log(`WASM dir:       ${WASM_DIR}`);
  console.log(`Manifest out:   ${MANIFEST_OUT}`);
  console.log("");

  const config = loadConfig(fs.readFileSync(CONFIG_FILE, "utf8"));

  const keypair = Keypair.fromSecret(DEPLOYER_SECRET);
  const server = new SorobanRpc.Server(RPC_URL);

  // Load existing manifest if present (for hash-based change detection)
  let existingManifest: Manifest | null = null;
  if (fs.existsSync(MANIFEST_OUT)) {
    existingManifest = JSON.parse(fs.readFileSync(MANIFEST_OUT, "utf8"));
    console.log(`Loaded existing manifest from ${MANIFEST_OUT}`);
  }

  const resolvedIds: Record<string, string> = {};
  // Seed from existing manifest so dependencies that haven't changed
  // can be referenced by contracts that do need deployment.
  if (existingManifest) {
    for (const [name, entry] of Object.entries(existingManifest.contracts)) {
      resolvedIds[name] = entry.contract_id;
    }
  }

  const manifestContracts: Record<string, ManifestEntry> = existingManifest
    ? { ...existingManifest.contracts }
    : {};

  const gitSha =
    process.env.GITHUB_SHA ??
    process.env.GIT_SHA ??
    crypto.randomBytes(4).toString("hex");

  const networkName =
    process.env.STELLAR_NETWORK ?? "unknown";

  for (const contractConfig of config.contracts) {
    if (contractConfig.enabled === false) {
      console.log(`  [skip] ${contractConfig.name} (disabled in config)`);
      continue;
    }

    console.log(`\n── ${contractConfig.name} ──`);

    const artifactPath = resolveArtifactPath(contractConfig.artifact);
    const wasmBuffer = fs.readFileSync(artifactPath);
    const wasmHash = sha256Hex(wasmBuffer);

    // Check whether WASM has changed since last deployment
    const existing = existingManifest?.contracts[contractConfig.name];
    if (existing && existing.wasm_hash === wasmHash) {
      console.log(
        `  [unchanged] WASM hash matches existing manifest — skipping deployment.`
      );
      resolvedIds[contractConfig.name] = existing.contract_id;
      continue;
    }

    // Upload WASM
    console.log(`  Uploading WASM (${wasmBuffer.length} bytes) ...`);
    const uploadedHash = await uploadWasm(server, keypair, wasmBuffer);
    console.log(`  WASM hash: ${uploadedHash}`);

    // Deploy contract
    console.log(`  Creating contract instance ...`);
    const contractId = await deployContract(
      server,
      keypair,
      uploadedHash,
      contractConfig,
      resolvedIds
    );
    console.log(`  Contract ID: ${contractId}`);
    resolvedIds[contractConfig.name] = contractId;

    // Initialize / constructor call
    if (
      contractConfig.deployment_mode === "initialize" &&
      contractConfig.initialize_function &&
      contractConfig.initialize_args
    ) {
      const args = interpolateArgs(
        contractConfig.initialize_args,
        resolvedIds
      );
      console.log(
        `  Calling ${contractConfig.initialize_function}() ...`
      );
      await initializeContract(
        server,
        keypair,
        contractId,
        contractConfig.initialize_function,
        args
      );
    } else if (
      contractConfig.deployment_mode === "constructor" &&
      contractConfig.constructor_args
    ) {
      const args = interpolateArgs(
        contractConfig.constructor_args,
        resolvedIds
      );
      console.log(`  Calling constructor() ...`);
      await initializeContract(
        server,
        keypair,
        contractId,
        "__constructor",
        args
      );
    }

    manifestContracts[contractConfig.name] = {
      contract_id: contractId,
      wasm_hash: uploadedHash,
      deployed_at: new Date().toISOString(),
      network: networkName,
    };

    console.log(`  ✓ ${contractConfig.name} deployed successfully.`);
  }

  // Write manifest
  const manifest: Manifest = {
    schema_version: 1,
    git_sha: gitSha,
    network: networkName,
    deployed_at: new Date().toISOString(),
    contracts: manifestContracts,
  };

  const manifestDir = path.dirname(MANIFEST_OUT);
  fs.mkdirSync(manifestDir, { recursive: true });
  fs.writeFileSync(MANIFEST_OUT, JSON.stringify(manifest, null, 2));

  console.log(`\n✓ Manifest written to ${MANIFEST_OUT}`);
  console.log("\nDeployed contract IDs:");
  for (const [name, entry] of Object.entries(manifestContracts)) {
    console.log(`  ${name}: ${entry.contract_id}`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((err) => {
    console.error("\nDeployment failed:", err);
    process.exit(1);
  });
}
