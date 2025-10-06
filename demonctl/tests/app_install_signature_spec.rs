use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use assert_cmd::Command;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use p256::ecdsa::{signature::Signer, Signature, SigningKey};
use p256::pkcs8::EncodePublicKey;
use pem_rfc7468::LineEnding;
use rand_core::OsRng;
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

#[test]
fn install_signed_pack_succeeds() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_signed_pack(&pack_dir, "signed-app")?;

    let install_home = temp.path().join("home");

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .success();

    let installed_manifest = install_home.join("packs/signed-app/0.1.0/app-pack.yaml");
    assert!(installed_manifest.exists());

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["run", "signed-app:noop"])
        .assert()
        .success();

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["run", "signed-app@0.1.0:noop"])
        .assert()
        .success();

    Ok(())
}

#[test]
fn install_with_tampered_signature_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_signed_pack(&pack_dir, "tampered-app")?;

    fs::write(pack_dir.join("signing/cosign.sig"), "AAAA")?;

    let install_home = temp.path().join("home");

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .failure();

    Ok(())
}

#[test]
fn install_with_tampered_manifest_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_signed_pack(&pack_dir, "tampered-manifest")?;

    let manifest_path = pack_dir.join("app-pack.yaml");
    let mut manifest_contents = fs::read_to_string(&manifest_path)?;
    manifest_contents.push_str("\n# tampered\n");
    fs::write(&manifest_path, manifest_contents)?;

    let install_home = temp.path().join("home");

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .failure();

    Ok(())
}

fn create_signed_pack(root: &Path, app_name: &str) -> Result<()> {
    fs::create_dir_all(root.join("contracts/test"))?;
    fs::create_dir_all(root.join("signing"))?;

    let signing_key = SigningKey::random(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let public_key_pem = verifying_key
        .to_public_key_pem(LineEnding::LF)
        .context("failed to encode public key")?;
    let public_key_path = root.join("signing/cosign.pub");
    fs::write(&public_key_path, public_key_pem.as_bytes())?;

    let hash = Sha256::digest(public_key_pem.as_bytes());
    let hash_hex = hex::encode(hash);

    write_contracts(root)?;

    let manifest_contents = build_manifest(app_name, &hash_hex);
    let manifest_path = root.join("app-pack.yaml");
    fs::write(&manifest_path, manifest_contents.as_bytes())?;

    let manifest_digest_hex = hex::encode(Sha256::digest(manifest_contents.as_bytes()));
    let signature: Signature = signing_key.sign(manifest_contents.as_bytes());
    let signature_der = signature.to_der().as_bytes().to_vec();
    let signature_b64 = BASE64.encode(signature_der);

    let hashed_rekor = json!({
        "apiVersion": "0.0.1",
        "kind": "rekord",
        "spec": {
            "data": {
                "hash": {
                    "algorithm": "sha256",
                    "value": manifest_digest_hex,
                }
            },
            "signature": {
                "content": signature_b64,
                "format": "x509",
                "publicKey": {
                    "content": BASE64.encode(public_key_pem.as_bytes()),
                },
            },
        }
    });

    let bundle = json!({
        "Payload": {
            "body": BASE64.encode(serde_json::to_vec(&hashed_rekor)?),
            "integratedTime": 0,
            "logIndex": 0,
            "logID": "local-development",
        },
        "SignedEntryTimestamp": BASE64.encode(b"local-set"),
    });

    let signature_path = root.join("signing/cosign.sig");
    fs::write(&signature_path, serde_json::to_string_pretty(&bundle)?)?;

    Ok(())
}

fn write_contracts(root: &Path) -> Result<()> {
    let contract_path = root.join("contracts/test/contract.json");
    fs::write(contract_path, r#"{"type":"object"}"#)?;
    Ok(())
}

fn build_manifest(app_name: &str, public_key_hash: &str) -> String {
    let manifest_json = json!({
        "apiVersion": "demon.io/v1",
        "kind": "AppPack",
        "metadata": {
            "name": app_name,
            "version": "0.1.0"
        },
        "signing": {
            "cosign": {
                "enabled": true,
                "signaturePath": "signing/cosign.sig",
                "publicKeyPath": "signing/cosign.pub",
                "publicKeyHash": {
                    "algorithm": "sha256",
                    "value": public_key_hash
                }
            }
        },
        "contracts": [
            {
                "id": format!("{app_name}/contract"),
                "version": "0.1.0",
                "path": "contracts/test/contract.json"
            }
        ],
        "capsules": [
            {
                "type": "container-exec",
                "name": "noop",
                "imageDigest": "ghcr.io/example/noop@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "command": ["/bin/true"],
                "outputs": {
                    "envelopePath": "/workspace/.artifacts/result.json"
                }
            }
        ],
        "rituals": [
            {
                "name": "noop",
                "steps": [
                    { "capsule": "noop" }
                ]
            }
        ]
    });

    let yaml = serde_yaml::to_string(&manifest_json).expect("failed to serialize manifest to YAML");
    yaml.trim_start_matches("---\n").to_string()
}
