# Changelog

## Unreleased

- demonctl: bump `sigstore` to 0.14 (aws-lc-rs backend), which drops the transitive `rsa` crate; remove the RUSTSEC-2023-0071 `cargo-audit` suppression
- docs: CI for bundle provenance — positive & negative offline verify; key rotation + determinism guidance
- demonctl: enforce Cosign signature verification (hashed key, bundle validation) and drop `--allow-unsigned`
- demonctl: add ritual alias support (`demonctl run <app>:<ritual>`) backed by installed App Packs
