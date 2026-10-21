from __future__ import annotations

import json
import os
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping
from unittest import mock

from deployment.models import (
    DeploymentConfig,
    DeploymentError,
    NetworkConfig,
    VerificationFailed,
    canonical_json,
    load_json_object,
    sha256_file,
)
from deployment.orchestrator import (
    DeploymentOrchestrator,
    partial_manifest_path,
    resolve_mapping,
    verify_manifest,
)


class FakeStellarClient:
    def __init__(self) -> None:
        self.calls: list[tuple[Any, ...]] = []
        self.remote_hashes: dict[str, str] = {}
        self.remote_metadata: dict[str, str] = {}
        self.fail_initialize_once = False
        self._contract_counter = 0

    def account_address(self, identity: str) -> str:
        self.calls.append(("address", identity))
        return "G" + "A" * 55

    def upload_wasm(
        self, artifact: Path, identity: str, network: NetworkConfig
    ) -> str:
        digest = sha256_file(artifact)
        self.calls.append(("upload", artifact.name, digest))
        return digest

    def deploy_contract(
        self,
        wasm_hash: str,
        identity: str,
        network: NetworkConfig,
        constructor_args: Mapping[str, Any],
    ) -> str:
        self._contract_counter += 1
        suffix = format(self._contract_counter, "X")
        contract_id = "C" + "A" * (55 - len(suffix)) + suffix
        self.remote_hashes[contract_id] = wasm_hash
        self.remote_metadata[contract_id] = canonical_json({"wasm_hash": wasm_hash})
        self.calls.append(
            ("deploy", wasm_hash, contract_id, dict(constructor_args))
        )
        return contract_id

    def invoke(
        self,
        contract_id: str,
        identity: str,
        network: NetworkConfig,
        function: str,
        args: Mapping[str, Any],
    ) -> str:
        self.calls.append(("invoke", contract_id, function, dict(args)))
        if function == "initialize" and self.fail_initialize_once:
            self.fail_initialize_once = False
            raise DeploymentError("fixture initialization failure")
        return "0"

    def contract_hash(self, contract_id: str, network: NetworkConfig) -> str:
        self.calls.append(("hash", contract_id))
        try:
            return self.remote_hashes[contract_id]
        except KeyError as exc:
            raise DeploymentError(f"unknown fixture contract: {contract_id}") from exc

    def contract_metadata(
        self,
        *,
        artifact: Path | None = None,
        contract_id: str | None = None,
        network: NetworkConfig | None = None,
    ) -> str:
        if artifact is not None:
            return canonical_json({"wasm_hash": sha256_file(artifact)})
        if contract_id is not None:
            try:
                return self.remote_metadata[contract_id]
            except KeyError as exc:
                raise DeploymentError(
                    f"unknown fixture contract: {contract_id}"
                ) from exc
        raise DeploymentError("fixture metadata subject missing")


class DeploymentOrchestratorTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.wasm_dir = self.root / "wasm"
        self.wasm_dir.mkdir()
        self._write_wasm("registry", b"registry fixture")
        self._write_wasm("asset", b"asset fixture")
        self.config_path = self.root / "deployment.json"
        self.config_path.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "profile": "development",
                    "contracts": [
                        {
                            "name": "asset",
                            "artifact": "${wasm_dir}/asset.wasm",
                            "env_key": "VITE_ASSET_ID",
                            "dependencies": ["registry"],
                            "deployment_mode": "constructor",
                            "constructor_args": {
                                "admin": "${admin}",
                                "registry": "${contract.registry}",
                                "meta": {"name": "fixture"},
                            },
                            "declared_metadata": {"role": "asset"},
                            "health_check": {
                                "function": "total_supply",
                                "expectation": "integer",
                            },
                        },
                        {
                            "name": "registry",
                            "artifact": "${wasm_dir}/registry.wasm",
                            "env_key": "VITE_REGISTRY_ID",
                            "dependencies": [],
                            "deployment_mode": "initialize",
                            "initialize_function": "initialize",
                            "initialize_args": {"admin": "${admin}"},
                            "declared_metadata": {"role": "registry"},
                            "health_check": {
                                "function": "entry_count",
                                "expectation": "integer",
                            },
                        },
                    ],
                }
            ),
            encoding="utf-8",
        )
        self.config = DeploymentConfig.load(self.config_path)
        self.network = NetworkConfig(name="testnet")
        self.manifest_path = self.root / "deploy-manifest.json"
        self.report_path = self.root / "verification-report.json"
        self.frontend_env = self.root / "frontend" / ".env"
        self.fixed_now = lambda: datetime(
            2026, 7, 29, 7, 0, 0, tzinfo=timezone.utc
        )

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_dependency_order_and_verified_atomic_outputs(self) -> None:
        client = FakeStellarClient()
        orchestrator = DeploymentOrchestrator(client, now=self.fixed_now)

        manifest, report = orchestrator.deploy(
            config=self.config,
            repo_root=self.root,
            wasm_dir=self.wasm_dir,
            identity="fixture",
            network=self.network,
            manifest_path=self.manifest_path,
            report_path=self.report_path,
            frontend_env_path=self.frontend_env,
        )

        operations = [
            call[0] for call in client.calls if call[0] in {"upload", "deploy", "invoke"}
        ]
        self.assertEqual(
            operations,
            [
                "upload",
                "deploy",
                "invoke",
                "upload",
                "deploy",
                "invoke",
                "invoke",
            ],
        )
        deployed_names = list(manifest["contracts"])
        self.assertEqual(deployed_names, ["registry", "asset"])
        self.assertEqual(manifest["status"], "verified")
        self.assertEqual(report["status"], "passed")
        self.assertFalse(partial_manifest_path(self.manifest_path).exists())
        self.assertTrue(self.manifest_path.exists())
        self.assertTrue(self.report_path.exists())
        self.assertEqual(report["manifest_sha256"], sha256_file(self.manifest_path))
        env = self.frontend_env.read_text(encoding="utf-8")
        self.assertIn("VITE_STELLAR_NETWORK=testnet", env)
        self.assertIn("VITE_REGISTRY_ID=C", env)
        self.assertIn("VITE_ASSET_ID=C", env)
        asset = manifest["contracts"]["asset"]
        self.assertEqual(
            asset["resolved_dependencies"]["registry"],
            manifest["contracts"]["registry"]["contract_id"],
        )

    def test_interrupted_initializer_resumes_without_redeploy(self) -> None:
        client = FakeStellarClient()
        client.fail_initialize_once = True
        orchestrator = DeploymentOrchestrator(client, now=self.fixed_now)
        kwargs = {
            "config": self.config,
            "repo_root": self.root,
            "wasm_dir": self.wasm_dir,
            "identity": "fixture",
            "network": self.network,
            "manifest_path": self.manifest_path,
            "report_path": self.report_path,
            "frontend_env_path": self.frontend_env,
        }

        with self.assertRaisesRegex(DeploymentError, "fixture initialization"):
            orchestrator.deploy(**kwargs)
        partial_path = partial_manifest_path(self.manifest_path)
        partial = load_json_object(partial_path)
        self.assertEqual(partial["contracts"]["registry"]["stage"], "deployed")
        upload_count = sum(call[0] == "upload" for call in client.calls)
        deploy_count = sum(call[0] == "deploy" for call in client.calls)

        manifest, _ = orchestrator.deploy(**kwargs, resume=True)

        self.assertEqual(
            sum(call[0] == "upload" for call in client.calls),
            upload_count + 1,
        )
        self.assertEqual(
            sum(call[0] == "deploy" for call in client.calls),
            deploy_count + 1,
        )
        self.assertEqual(manifest["status"], "verified")
        self.assertFalse(partial_path.exists())

    def test_verification_detects_remote_code_hash_mismatch(self) -> None:
        client = FakeStellarClient()
        orchestrator = DeploymentOrchestrator(client, now=self.fixed_now)
        manifest, _ = orchestrator.deploy(
            config=self.config,
            repo_root=self.root,
            wasm_dir=self.wasm_dir,
            identity="fixture",
            network=self.network,
            manifest_path=self.manifest_path,
            report_path=self.report_path,
            frontend_env_path=self.frontend_env,
        )
        asset_id = manifest["contracts"]["asset"]["contract_id"]
        client.remote_hashes[asset_id] = "f" * 64

        report = verify_manifest(
            manifest=manifest,
            repo_root=self.root,
            client=client,
            identity="fixture",
            generated_at="2026-07-29T07:01:00Z",
        )

        self.assertEqual(report["status"], "failed")
        mismatch = [
            check
            for check in report["checks"]
            if check["contract"] == "asset"
            and check["check"] == "deployed_wasm_hash"
        ][0]
        self.assertEqual(mismatch["status"], "failed")

    def test_verification_failure_keeps_previous_registry(self) -> None:
        client = FakeStellarClient()
        orchestrator = DeploymentOrchestrator(client, now=self.fixed_now)
        self.manifest_path.write_text(
            json.dumps({"generation": 7, "sentinel": "keep-me"}),
            encoding="utf-8",
        )
        original = self.manifest_path.read_text(encoding="utf-8")

        original_metadata = client.contract_metadata

        def mismatched_metadata(**kwargs: Any) -> str:
            if kwargs.get("contract_id"):
                return canonical_json({"mismatch": True})
            return original_metadata(**kwargs)

        client.contract_metadata = mismatched_metadata  # type: ignore[method-assign]
        with self.assertRaises(VerificationFailed):
            orchestrator.deploy(
                config=self.config,
                repo_root=self.root,
                wasm_dir=self.wasm_dir,
                identity="fixture",
                network=self.network,
                manifest_path=self.manifest_path,
                report_path=self.report_path,
                frontend_env_path=self.frontend_env,
            )

        self.assertEqual(
            self.manifest_path.read_text(encoding="utf-8"),
            original,
        )
        self.assertTrue(partial_manifest_path(self.manifest_path).exists())
        self.assertEqual(load_json_object(self.report_path)["status"], "failed")

    def test_resume_skips_completed_stage_and_starts_at_first_incomplete(
        self,
    ) -> None:
        client = FakeStellarClient()
        original_upload = client.upload_wasm

        def fail_asset_upload(
            artifact: Path, identity: str, network: NetworkConfig
        ) -> str:
            if artifact.name == "asset.wasm":
                raise DeploymentError("fixture asset upload failure")
            return original_upload(artifact, identity, network)

        client.upload_wasm = fail_asset_upload  # type: ignore[method-assign]
        orchestrator = DeploymentOrchestrator(client, now=self.fixed_now)
        kwargs = {
            "config": self.config,
            "repo_root": self.root,
            "wasm_dir": self.wasm_dir,
            "identity": "fixture",
            "network": self.network,
            "manifest_path": self.manifest_path,
            "report_path": self.report_path,
            "frontend_env_path": self.frontend_env,
        }

        with self.assertRaisesRegex(DeploymentError, "fixture asset upload"):
            orchestrator.deploy(**kwargs)
        partial = load_json_object(partial_manifest_path(self.manifest_path))
        registry_id = partial["contracts"]["registry"]["contract_id"]
        self.assertEqual(partial["contracts"]["registry"]["stage"], "initialized")
        self.assertEqual(partial["contracts"]["asset"]["stage"], "pending")

        client.upload_wasm = original_upload  # type: ignore[method-assign]
        client.calls.clear()
        manifest, _ = orchestrator.deploy(**kwargs, resume=True)

        mutating = [
            call for call in client.calls if call[0] in {"upload", "deploy", "invoke"}
        ]
        self.assertEqual(mutating[0][:2], ("upload", "asset.wasm"))
        self.assertEqual(mutating[1][0], "deploy")
        self.assertFalse(
            any(call[0] == "upload" and call[1] == "registry.wasm" for call in mutating)
        )
        self.assertEqual(sum(call[0] == "deploy" for call in mutating), 1)
        self.assertFalse(
            any(
                call[0] == "invoke" and call[2] == "initialize"
                for call in mutating
            )
        )
        self.assertEqual(manifest["contracts"]["registry"]["contract_id"], registry_id)
        self.assertEqual(
            manifest["contracts"]["asset"]["resolved_dependencies"]["registry"],
            registry_id,
        )
        self.assertEqual(manifest["status"], "verified")

    def test_failed_stage_command_preserves_previous_manifest(self) -> None:
        client = FakeStellarClient()
        orchestrator = DeploymentOrchestrator(client, now=self.fixed_now)
        kwargs = {
            "config": self.config,
            "repo_root": self.root,
            "wasm_dir": self.wasm_dir,
            "identity": "fixture",
            "network": self.network,
            "manifest_path": self.manifest_path,
            "report_path": self.report_path,
            "frontend_env_path": self.frontend_env,
        }
        orchestrator.deploy(**kwargs)
        original_bytes = self.manifest_path.read_bytes()
        original_data = load_json_object(self.manifest_path)

        def failing_deploy(*args: Any, **kwargs: Any) -> str:
            raise DeploymentError("fixture contract deploy command failed")

        client.deploy_contract = failing_deploy  # type: ignore[method-assign]
        with self.assertRaisesRegex(DeploymentError, "deploy command failed"):
            orchestrator.deploy(**kwargs)

        self.assertEqual(self.manifest_path.read_bytes(), original_bytes)
        self.assertEqual(load_json_object(self.manifest_path), original_data)
        self.assertEqual(original_data["status"], "verified")
        self.assertTrue(partial_manifest_path(self.manifest_path).exists())

    def test_failed_manifest_write_preserves_previous_manifest(self) -> None:
        client = FakeStellarClient()
        orchestrator = DeploymentOrchestrator(client, now=self.fixed_now)
        kwargs = {
            "config": self.config,
            "repo_root": self.root,
            "wasm_dir": self.wasm_dir,
            "identity": "fixture",
            "network": self.network,
            "manifest_path": self.manifest_path,
            "report_path": self.report_path,
            "frontend_env_path": self.frontend_env,
        }
        orchestrator.deploy(**kwargs)
        original_bytes = self.manifest_path.read_bytes()
        manifest_path = self.manifest_path
        real_replace = os.replace

        def failing_replace(src: Any, dst: Any) -> None:
            if Path(dst) == manifest_path:
                raise OSError("fixture manifest replace failure")
            real_replace(src, dst)

        with mock.patch("deployment.models.os.replace", side_effect=failing_replace):
            with self.assertRaisesRegex(OSError, "manifest replace failure"):
                orchestrator.deploy(**kwargs)

        self.assertEqual(self.manifest_path.read_bytes(), original_bytes)
        self.assertEqual(load_json_object(self.manifest_path)["status"], "verified")
        leftovers = list(self.root.glob(f".{self.manifest_path.name}.*.tmp"))
        self.assertEqual(leftovers, [])

    def test_missing_artifact_is_reported_before_upload(self) -> None:
        (self.wasm_dir / "asset.wasm").unlink()
        client = FakeStellarClient()
        orchestrator = DeploymentOrchestrator(client, now=self.fixed_now)
        with self.assertRaisesRegex(DeploymentError, "WASM artifact not found"):
            orchestrator.deploy(
                config=self.config,
                repo_root=self.root,
                wasm_dir=self.wasm_dir,
                identity="fixture",
                network=self.network,
                manifest_path=self.manifest_path,
                report_path=self.report_path,
                frontend_env_path=self.frontend_env,
            )
        self.assertFalse(any(call[0] == "upload" for call in client.calls))

    def test_mainnet_rejects_development_profile(self) -> None:
        with self.assertRaisesRegex(DeploymentError, "profile='production'"):
            self.config.validate_for_network(NetworkConfig(name="mainnet"))

    def test_template_resolution_handles_nested_metadata(self) -> None:
        resolved = resolve_mapping(
            {
                "admin": "${admin}",
                "meta": {
                    "registry": "${contract.registry}",
                    "label": "admin=${admin}",
                },
            },
            {
                "admin": "GADMIN",
                "contract.registry": "CREGISTRY",
            },
        )
        self.assertEqual(
            resolved,
            {
                "admin": "GADMIN",
                "meta": {
                    "registry": "CREGISTRY",
                    "label": "admin=GADMIN",
                },
            },
        )

    def _write_wasm(self, name: str, payload: bytes) -> None:
        (self.wasm_dir / f"{name}.wasm").write_bytes(
            b"\x00asm\x01\x00\x00\x00" + payload
        )


class DeploymentConfigTest(unittest.TestCase):
    def test_dependency_cycle_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "config.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "contracts": [
                            {
                                "name": "a",
                                "artifact": "a.wasm",
                                "env_key": "A",
                                "dependencies": ["b"],
                                "deployment_mode": "constructor",
                            },
                            {
                                "name": "b",
                                "artifact": "b.wasm",
                                "env_key": "B",
                                "dependencies": ["a"],
                                "deployment_mode": "constructor",
                            },
                        ],
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(DeploymentError, "dependency cycle"):
                DeploymentConfig.load(path)


if __name__ == "__main__":
    unittest.main()
