# App Packs

**Portable, versioned bundles for distributing Demon applications.**

## Overview

App Packs provide a standardized way to package, distribute, and install applications on the Demon platform. Each pack bundles:

- **Contracts**: JSON Schema definitions that validate data flowing through rituals
- **Capsules**: Container-based execution units with sandbox configurations
- **Rituals**: Workflow definitions that orchestrate capsule execution
- **UI Cards**: Display configurations for the Operate UI dashboard
- **Signatures**: Optional cryptographic signatures for verification (Cosign)

App Packs enable teams to share reusable automation workflows while maintaining strong contracts and security guarantees.

## Schema Reference

App Packs are defined using the **App Pack v1 schema** located at:

```
contracts/schemas/app-pack.v1.schema.json
```

The schema enforces:
- Semantic versioning for all components
- DNS-compatible naming conventions
- Relative paths for security (no absolute paths or `..` traversal)
- Required fields for reproducible installations
- Optional signature verification settings

See the [schema file](../contracts/schemas/app-pack.v1.schema.json) for the complete specification.

## Manifest Structure

An App Pack manifest (`app-pack.yaml` or `app-pack.yml`) contains:

### Metadata

```yaml
apiVersion: demon.io/v1
kind: AppPack
metadata:
  name: my-app              # DNS-compatible slug (required)
  version: 1.0.0            # Semantic version (required)
  displayName: My Application
  description: Optional application description
  repository: https://github.com/org/repo
  license: Apache-2.0
  homepage: https://example.com
```

### Compatibility & Requirements

```yaml
compatibility:
  appPackSchema: ">=1.0.0 <2.0.0"
  platformAPI: ">=0.1.0"

requires:
  appPackSchema: ">=1.0.0"
  platformApis:
    engine: ">=0.1.0"
    runtime: ">=0.1.0"
    operateUi: ">=0.1.0"
```

### Contracts

```yaml
contracts:
  - id: my-app/request
    version: 1.0.0
    path: contracts/my-request.v1.json
  - id: my-app/response
    version: 1.0.0
    path: contracts/my-response.v1.json
```

Contract paths must be relative to the pack root and cannot use `..` for security.

### Capsules

```yaml
capsules:
  - type: container-exec
    name: my-capsule
    imageDigest: ghcr.io/org/image@sha256:abc123...
    command: ["/app/main"]
    env:
      APP_MODE: production
    workingDir: /workspace
    timeoutSeconds: 300
    outputs:
      envelopePath: /workspace/.artifacts/result.json
    sandbox:
      network: none
      readOnly: false
      tmpfs:
        - /tmp
      securityOpt:
        - no-new-privileges:true
```

**Key fields:**
- `imageDigest`: Must be a digest-pinned reference (`@sha256:...`)
- `outputs.envelopePath`: Where the capsule writes its result envelope
- `sandbox`: Security constraints applied by the runtime

### Rituals

```yaml
rituals:
  - name: my-workflow
    displayName: My Workflow
    description: Does something useful
    steps:
      - capsule: my-capsule
        with:
          param1: value1
          param2: value2
```

Each ritual defines a sequence of capsule invocations. The `with` block passes configuration to the capsule.

### UI Cards

```yaml
ui:
  cards:
    # Result summary with status badges
    - id: my-result-card
      kind: result-envelope
      title: Execution Summary
      description: Shows outcome, duration, and status
      match:
        rituals:
          - my-workflow
      config:
        statusPath: result.success
        durationPath: duration
        showTimestamp: true

    # Structured field display
    - id: my-fields-card
      kind: fields-table
      title: Key Outputs
      description: Important fields from the execution
      match:
        rituals:
          - my-workflow
      config:
        fields:
          - label: Status
            path: result.success
            format: badge
          - label: Message
            path: result.data.message
            format: text

    # Full JSON inspection
    - id: my-json-card
      kind: json-viewer
      title: Complete Output
      description: Full ritual output for debugging
      match:
        rituals:
          - my-workflow
      config:
        expandDepth: 2
```

UI cards define how Operate UI displays ritual execution results. The Operate UI supports four card types:

- **result-envelope**: Status badges, duration, timestamps, and markdown summaries
- **fields-table**: Configurable key-value table with formatted values
- **markdown-view**: Long-form text or log content with scrolling
- **json-viewer**: Full JSON output inspection with syntax highlighting

Cards are matched to rituals by name and automatically rendered when viewing run details. Multiple cards can be defined for the same ritual to show different aspects of the output.

**See [UI Manifests Guide](./ui-manifests.md) for complete card type specifications, JSON path syntax, and examples.**

### Signature Verification (Optional)

```yaml
signing:
  cosign:
    enabled: true
    signaturePath: signing/cosign.sig
    publicKeyPath: signing/cosign.pub
    publicKeyHash:
      algorithm: sha256
      value: abc123...
```

When signing is enabled, `demonctl app install` verifies the manifest signature before installation.

## CLI Commands

### Install

```bash
# Install from a directory
demonctl app install path/to/app-pack

# Install from a manifest file
demonctl app install path/to/app-pack.yaml

# Overwrite existing installation
demonctl app install --overwrite path/to/app-pack
```

**Installation process:**
1. Resolves the pack source (directory or manifest file)
2. Parses and validates the manifest against the schema
3. Verifies signature if signing is enabled
4. Checks for existing installations (fails unless `--overwrite` is used)
5. Copies the entire pack directory to `~/.demon/app-packs/packs/<name>/<version>`
6. Validates that all referenced contracts exist
7. Registers the pack in the local registry (`~/.demon/app-packs/registry.json`)

**Idempotency:**
- Re-installing the same version without `--overwrite` fails with a clear error
- With `--overwrite`, the pack is removed and reinstalled cleanly

### List

```bash
# Human-readable table
demonctl app list

# JSON output for automation
demonctl app list --json
```

Output shows:
- Pack name and version
- Installation timestamp
- Source path (where it was installed from)
- Schema compatibility range

### Uninstall

```bash
# Remove pack and all versions
demonctl app uninstall my-app

# Remove specific version
demonctl app uninstall my-app --version 1.0.0

# Remove registry entry but keep files
demonctl app uninstall my-app --retain-files
```

**Cleanup process:**
1. Removes registry entry
2. Deletes pack files from `~/.demon/app-packs/packs/<name>/<version>` (unless `--retain-files`)
3. Cleans up empty name directories

## Running Rituals from App Packs

Once installed, rituals can be executed using the **alias syntax**:

```bash
# Using pack name and ritual name
demonctl run <pack-name>:<ritual-name>

# Using specific version
demonctl run <pack-name>@<version>:<ritual-name>
```

**Examples:**

```bash
# Run the 'hello' ritual from the 'hello-world' pack
demonctl run hello-world:hello

# Run from a specific version
demonctl run hello-world@1.0.0:hello
```

The CLI resolves the alias by:
1. Looking up the pack in the registry
2. Finding the installed manifest
3. Extracting the ritual definition
4. Generating a temporary ritual file
5. Executing the ritual via the engine

## End-to-End Example

### 1. Create an App Pack

Start with the example pack:

```bash
cd examples/app-pack-sample
ls -la
# app-pack.yaml
# contracts/
# README.md
```

### 2. Install the Pack

```bash
demonctl app install examples/app-pack-sample
# Output: Installed App Pack hello-world@1.0.0
```

### 3. Verify Installation

```bash
demonctl app list
# NAME         VERSION      INSTALLED                 SOURCE
# hello-world  1.0.0        2024-11-01T12:00:00Z      examples/app-pack-sample
```

### 4. Run a Ritual

```bash
demonctl run hello-world:hello
# Executes the 'hello' ritual from the pack
# Output includes envelope with ritual.started and ritual.completed events
```

### 5. View in Operate UI

Navigate to the Operate UI (`http://localhost:3000`) to see:
- Run history for the hello-world ritual
- UI card displaying the output (configured in the pack's `ui.cards` section)
- Timeline of envelope events

### 6. Uninstall

```bash
demonctl app uninstall hello-world
# Output: Uninstalled hello-world@1.0.0
```

## Security & Signing

### Cosign Integration

App Packs support **Cosign** for cryptographic signature verification:

1. **Generate a key pair**:
   ```bash
   cosign generate-key-pair
   # Creates cosign.key and cosign.pub
   ```

2. **Sign the manifest**:
   ```bash
   cosign sign-blob \
     --key cosign.key \
     --bundle signing/cosign.sig \
     app-pack.yaml
   ```

3. **Configure signing in manifest**:
   ```yaml
   signing:
     cosign:
       enabled: true
       signaturePath: signing/cosign.sig
       publicKeyPath: signing/cosign.pub
       publicKeyHash:
         algorithm: sha256
         value: <sha256-of-pub-key-pem>
   ```

4. **Install verifies automatically**:
   ```bash
   demonctl app install my-signed-pack
   # Verifies signature before installation
   # Fails if signature is invalid or manifest is tampered
   ```

### Security Best Practices

- **Always use digest-pinned images**: `image@sha256:...` instead of tags
- **Enable signature verification** for production packs
- **Use sandbox settings** to restrict capsule capabilities:
  - `network: none` for offline operations
  - `readOnly: true` for immutable root filesystem
  - `securityOpt: ["no-new-privileges:true"]` to prevent privilege escalation
- **Validate contracts** with JSON Schema to catch malformed data
- **Never commit secrets** to pack manifests (use runtime secrets injection instead)

## Registry & Storage

### Local Registry

App Packs are tracked in `~/.demon/app-packs/registry.json`:

```json
{
  "hello-world": [
    {
      "name": "hello-world",
      "version": "1.0.0",
      "installed_at": "2024-11-01T12:00:00Z",
      "manifest_path": "/home/user/.demon/app-packs/packs/hello-world/1.0.0/app-pack.yaml",
      "source": "examples/app-pack-sample",
      "schema_range": ">=1.0.0"
    }
  ]
}
```

The registry enables:
- Version tracking (multiple versions of the same pack)
- Fast lookups for alias resolution
- Metadata persistence (installation timestamp, source)

### File Layout

```
~/.demon/app-packs/
├── registry.json                          # Pack registry
└── packs/                                 # Installed packs
    └── <pack-name>/
        └── <version>/
            ├── app-pack.yaml              # Manifest
            ├── contracts/                 # Contracts
            │   └── ...
            ├── signing/                   # Signatures (if present)
            │   ├── cosign.pub
            │   └── cosign.sig
            └── ...                        # Other pack files
```

### Environment Variables

- `DEMON_APP_HOME`: Override the app packs directory (default: `~/.demon/app-packs`)
- `DEMON_HOME`: Base directory for all Demon data (app packs use `$DEMON_HOME/app-packs`)
- `HOME`: Fallback if neither above is set

## Contract Validation

Contracts in App Packs are validated against the **contracts-validate** CI check:

```bash
# Validate all schemas locally
scripts/contracts-validate.sh

# Includes app-pack.v1.schema.json and all pack contracts
```

This ensures:
- All JSON Schemas are syntactically valid
- Contracts follow the schema specification
- No backward-incompatible changes are introduced

## Troubleshooting

### Installation Fails with "Already Installed"

**Cause**: The pack version is already installed.

**Solution**: Use `--overwrite` to replace it, or uninstall first:

```bash
demonctl app install --overwrite path/to/pack
# OR
demonctl app uninstall my-app
demonctl app install path/to/pack
```

### Signature Verification Fails

**Cause**: Manifest was modified, signature is invalid, or public key hash mismatch.

**Solution**: Re-sign the manifest or verify the public key:

```bash
# Verify public key hash matches
sha256sum signing/cosign.pub
# Compare to publicKeyHash.value in manifest

# Re-sign if needed
cosign sign-blob --key cosign.key --bundle signing/cosign.sig app-pack.yaml
```

### Missing Contracts

**Cause**: Manifest references a contract that doesn't exist in the pack directory.

**Solution**: Ensure all contract paths are correct and files exist:

```bash
# Check referenced paths
grep 'path: contracts/' app-pack.yaml
# Verify files exist
ls -la contracts/
```

### Alias Not Found

**Cause**: Pack isn't installed or registry is corrupted.

**Solution**: Verify installation and re-install if needed:

```bash
demonctl app list
# If missing, reinstall:
demonctl app install path/to/pack
```

## Testing

### Local Compat Smoke Test

The `app-pack-compat-smoke` CI job validates the end-to-end flow: install → run → render. You can run these steps locally to verify changes:

**Prerequisites:**
```bash
# Start NATS JetStream
make dev
# Or manually:
docker run -d --rm --name nats -p 4222:4222 -p 8222:8222 nats:2.10 -js
```

**Run the test sequence:**

```bash
# 1. Install the sample app pack
cargo run -p demonctl -- app install examples/app-pack-sample/

# 2. List installed packs
cargo run -p demonctl -- app list

# 3. Run the hello ritual
export RITUAL_STREAM_NAME=RITUAL_EVENTS
cargo run -p demonctl -- run hello-world:hello

# 4. Start Operate UI (in another terminal)
RITUAL_STREAM_NAME=RITUAL_EVENTS cargo run -p operate-ui

# 5. View results
curl http://localhost:3000/api/runs | jq .

# 6. Cleanup
cargo run -p demonctl -- app uninstall hello-world
```

**Expected behavior:**
- Install succeeds with "Installed App Pack hello-world@1.0.0"
- Run completes with `"event": "ritual.completed:v1"` and `"success": true`
- UI `/api/runs` returns an array with the run
- Uninstall removes pack files and registry entry

**Troubleshooting:**
- If NATS is not running, ritual execution will fail with connection errors
- If Operate UI shows empty runs, verify `RITUAL_STREAM_NAME` matches across commands
- For detailed logs, add `RUST_LOG=debug` environment variable

## Future Enhancements

Planned features for App Packs:

- **Remote installation**: `demonctl app install https://example.com/pack.tar.gz`
- **OCI registry support**: `demonctl app install oci://ghcr.io/org/pack:v1.0.0`
- **Dependency resolution**: Packs that depend on other packs
- **Upgrade workflows**: `demonctl app upgrade <name>` to fetch and install latest version
- **Namespace isolation**: Multi-tenant pack installations

## References

- [App Pack Schema](../contracts/schemas/app-pack.v1.schema.json) — Complete schema definition
- [Sample Pack](../examples/app-pack-sample/) — Working example with contracts and rituals
- [Bootstrapper Bundles](bootstrapper/bundles.md) — Related bundle and signature concepts
- [Contract Registry](contracts/releases.md) — Contract versioning and distribution
- [Cosign Documentation](https://docs.sigstore.dev/cosign/overview/) — Signature tooling

## Contributing

To contribute improvements to App Pack functionality:

1. Review the [schema](../contracts/schemas/app-pack.v1.schema.json) for constraints
2. Ensure changes maintain backward compatibility
3. Update tests in `demonctl/tests/app_pack_*_spec.rs`
4. Add examples to the [sample pack](../examples/app-pack-sample/)
5. Update this documentation
6. Open a PR with `area:backend` and `story` labels

For questions or feedback, open a [discussion](https://github.com/afewell-hh/demon/discussions) or [issue](https://github.com/afewell-hh/demon/issues).
