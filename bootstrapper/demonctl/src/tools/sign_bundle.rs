#[cfg(feature = "dev-tools")]
fn main() {
    use base64::Engine;
    use bootstrapper_demonctl::bundle::canonicalize_bundle_to_bytes;
    use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
    use sha2::{Digest, Sha256};
    use std::env;
    use std::fs;
    use std::path::Path;
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: sign_bundle <bundle_yaml_path>");
        std::process::exit(2);
    }
    let bundle_path = Path::new(&args[1]);
    let bytes = canonicalize_bundle_to_bytes(bundle_path).expect("canonicalize");
    let mut h = Sha256::new();
    h.update(&bytes);
    let digest_hex = hex::encode(h.finalize());
    let sk_text =
        fs::read_to_string("tooling/testkeys/preview.ed25519.secret").expect("read secret");
    let sk_b64 = sk_text
        .lines()
        .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .expect("secret line");
    let sk_bytes = base64::engine::general_purpose::STANDARD
        .decode(sk_b64.trim())
        .expect("decode secret");
    let sk = SigningKey::from_bytes(sk_bytes.as_slice().try_into().expect("len 32"));
    let sig = sk.sign(&bytes);
    let vk: VerifyingKey = sk.verifying_key();
    let pub_b64 = base64::engine::general_purpose::STANDARD.encode(vk.to_bytes());
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
    println!("sha256={}", digest_hex);
    println!("pub_b64={}", pub_b64);
    println!("sig_b64={}", sig_b64);
}

#[cfg(not(feature = "dev-tools"))]
fn main() {}
