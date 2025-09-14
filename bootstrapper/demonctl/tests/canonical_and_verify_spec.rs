use std::path::{Path, PathBuf};

fn repo_path(rel: &str) -> PathBuf {
    let candidates = [
        Path::new(rel).to_path_buf(),
        Path::new("..").join(rel),
        Path::new("../..").join(rel),
        Path::new("../../..").join(rel),
    ];
    for p in candidates {
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(rel)
}

#[test]
fn canonicalization_is_stable_golden_digest() {
    let bundle = repo_path("examples/bundles/local-dev.yaml");
    let bytes = bootstrapper_demonctl::bundle::canonicalize_bundle_to_bytes(&bundle).unwrap();
    let digest = bootstrapper_demonctl::provenance::compute_digest_hex(&bytes);
    assert_eq!(
        digest,
        "f691d7f0acf56b000bea35321d5dcdfcdc56a0f2f033f49840b86e2438d59445"
    );
}

#[test]
fn signature_verifies_ok() {
    // Resolve via library index
    let idx = repo_path("bootstrapper/library/index.json");
    let resolved =
        bootstrapper_demonctl::libindex::resolve_local("lib://local/preview-local-dev@0.0.1", &idx)
            .unwrap();
    let vr = bootstrapper_demonctl::provenance::verify_provenance(
        &resolved.path,
        &resolved.pub_key_id,
        &resolved.digest_sha256,
        &resolved.sig_ed25519,
    )
    .unwrap();
    assert!(vr.signature_ok, "signature should verify");
}

#[test]
fn signature_rejects_on_tamper() {
    use base64::{engine::general_purpose, Engine as _};
    use ed25519_dalek::{Signature, VerifyingKey};
    // Load canonical bytes and signature, then flip a byte and verify directly
    let bundle = repo_path("examples/bundles/local-dev.yaml");
    let mut bytes = bootstrapper_demonctl::bundle::canonicalize_bundle_to_bytes(&bundle).unwrap();
    let idx = repo_path("bootstrapper/library/index.json");
    let resolved =
        bootstrapper_demonctl::libindex::resolve_local("lib://local/preview-local-dev@0.0.1", &idx)
            .unwrap();
    let pk_path = repo_path(&format!(
        "contracts/keys/{}.ed25519.pub",
        resolved.pub_key_id
    ));
    let pk_b64 = std::fs::read_to_string(pk_path).unwrap();
    let pk_bytes = general_purpose::STANDARD_NO_PAD
        .decode(pk_b64.trim())
        .or_else(|_| general_purpose::STANDARD.decode(pk_b64.trim()))
        .unwrap();
    let vk = VerifyingKey::from_bytes(pk_bytes.as_slice().try_into().unwrap()).unwrap();
    let sig_bytes = general_purpose::STANDARD_NO_PAD
        .decode(&resolved.sig_ed25519)
        .or_else(|_| general_purpose::STANDARD.decode(&resolved.sig_ed25519))
        .unwrap();
    let sig = Signature::from_slice(&sig_bytes).unwrap();
    // Sanity: original verifies
    assert!(vk.verify_strict(&bytes, &sig).is_ok());
    // Tamper one byte
    bytes[0] ^= 0x01;
    assert!(
        vk.verify_strict(&bytes, &sig).is_err(),
        "tampered bytes must fail"
    );
}
