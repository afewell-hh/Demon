# Hello World App Pack

A minimal example App Pack demonstrating the Demon platform's App Pack functionality.

## Overview

This App Pack provides a simple "hello world" ritual that can be installed and executed via `demonctl`. It demonstrates:

- **App Pack Manifest**: Complete `app-pack.yaml` with metadata, contracts, capsules, and rituals
- **Contract Definitions**: JSON Schema contracts for request and response validation
- **Capsule Configuration**: Container-based capsule with sandbox settings
- **Ritual Definition**: Simple single-step ritual workflow
- **UI Cards**: Operate UI card configuration for displaying ritual outputs

## Installation

Install this App Pack locally:

```bash
# From the repository root
demonctl app install examples/app-pack-sample

# Or specify the manifest directly
demonctl app install examples/app-pack-sample/app-pack.yaml
```

## Usage

Once installed, you can execute the hello ritual in several ways:

```bash
# Using the app pack alias (name:ritual format)
demonctl run hello-world:hello

# Using the versioned alias (name@version:ritual format)
demonctl run hello-world@1.0.0:hello

# Using a direct ritual file (if you have one)
# demonctl run path/to/hello-ritual.yaml
```

## Listing Installed Packs

View all installed App Packs:

```bash
# Human-readable format
demonctl app list

# JSON output for automation
demonctl app list --json
```

## Uninstalling

Remove the App Pack when you're done:

```bash
# Remove the pack and its files
demonctl app uninstall hello-world

# Remove only the registry entry, keep files
demonctl app uninstall hello-world --retain-files

# Remove a specific version
demonctl app uninstall hello-world --version 1.0.0
```

## Structure

```
examples/app-pack-sample/
├── app-pack.yaml                           # App Pack manifest
├── contracts/                               # Contract definitions
│   └── hello/
│       ├── hello-request.v1.json           # Request schema
│       └── hello-response.v1.json          # Response schema
├── signing/                                 # (Optional) Signature files
└── README.md                                # This file
```

## App Pack Manifest

The `app-pack.yaml` manifest defines:

- **Metadata**: Name, version, description, repository, license
- **Compatibility**: Supported version ranges for schema and platform APIs
- **Contracts**: JSON Schema definitions for data validation
- **Capsules**: Container-based execution units with sandbox settings
- **Rituals**: Workflow definitions that orchestrate capsule execution
- **UI Cards**: Display configurations for the Operate UI

## Contracts

This pack includes two contracts:

1. **hello-request.v1.json**: Validates input to the hello capsule
   - Required `message` field (string, 1-500 characters)
   - Optional `includeTimestamp` flag
   - Optional `metadata` object

2. **hello-response.v1.json**: Validates output from the hello capsule
   - Required `message` field
   - Optional `timestamp` (ISO 8601 date-time)
   - Optional `metadata` with capsule name and execution time

## Next Steps

- Explore the [App Pack schema](../../contracts/schemas/app-pack.v1.schema.json) for all available options
- Read the [App Pack documentation](../../docs/app-packs.md) for advanced features
- Create your own App Pack by copying this example and modifying it
- Add signature verification using Cosign (see the signing section in the schema)

## Notes

- The imageDigest in the manifest uses a placeholder SHA. In a real App Pack, this should reference an actual published container image.
- Signature verification is not enabled for this example. For production use, add the `signing.cosign` section to the manifest and include signature files in the `signing/` directory.
