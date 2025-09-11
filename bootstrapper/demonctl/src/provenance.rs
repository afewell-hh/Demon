use crate::bundle::canonicalize_bundle_to_bytes;
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub struct VerifyResult {
    pub digest_hex: String,
    pub signature_ok: bool,
    pub reason: Option<String>,
}

pub fn compute_digest_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    hex::encode(out)
}

pub fn verify_provenance(
    bundle_path: &Path,
    pubkey_id: &str,
    expected_digest_hex: &str,
    sig_b64: &str,
) -> Result<VerifyResult> {
    // Canonicalize
    let canon = canonicalize_bundle_to_bytes(bundle_path)?;
    let digest = compute_digest_hex(&canon);
    let mut ok = false;
    let mut reason: Option<String> = None;
    if digest != expected_digest_hex {
        reason = Some("digest-mismatch".to_string());
    } else {
        // Load pubkey
        let mut pk_path = Path::new("contracts/keys").join(format!("{}.ed25519.pub", pubkey_id));
        if !pk_path.exists() {
            for prefix in ["..", "../..", "../../.."].iter() {
                let p = Path::new(prefix).join(&pk_path);
                if p.exists() {
                    pk_path = p;
                    break;
                }
            }
        }
        let pk_b64 = fs::read_to_string(&pk_path)
            .with_context(|| format!("read pubkey: {}", pk_path.display()))?;
        let pk_trim = pk_b64.trim();
        let pk_bytes = general_purpose::STANDARD_NO_PAD
            .decode(pk_trim)
            .or_else(|_| general_purpose::STANDARD.decode(pk_trim))
            .context("decode pubkey base64")?;
        let vk = VerifyingKey::from_bytes(
            pk_bytes
                .as_slice()
                .try_into()
                .map_err(|_| anyhow::anyhow!("pubkey length"))?,
        )?;
        let sig_trim = sig_b64.trim();
        let sig_bytes = general_purpose::STANDARD_NO_PAD
            .decode(sig_trim)
            .or_else(|_| general_purpose::STANDARD.decode(sig_trim))
            .context("decode signature base64")?;
        let sig = Signature::from_slice(&sig_bytes)?;
        ok = vk.verify_strict(&canon, &sig).is_ok();
        if !ok {
            reason = Some("signature-invalid".to_string());
        }
    }
    Ok(VerifyResult {
        digest_hex: digest,
        signature_ok: ok,
        reason,
    })
}
