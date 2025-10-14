## demonctl batch

Run a series of rituals from a YAML file and optionally persist result envelopes.

### Batch File Format

YAML list of items with a `target` field:

```yaml
# docs/examples/batch.yaml
- target: examples/rituals/echo.yaml
- target: examples/rituals/timer.yaml
# You can also use installed App Pack aliases (if installed):
# - target: hoss:noop
```

### Commands

```bash
# Dry run (no envelope files written)
cargo run -p demonctl -- batch docs/examples/batch.yaml

# Save envelopes next to the batch file
cargo run -p demonctl -- batch docs/examples/batch.yaml --save

# Save envelopes to a custom directory (implies --save)
cargo run -p demonctl -- batch docs/examples/batch.yaml --save-dir ./.artifacts
```

Output files are named `batch-XXX.result.json` (zero‑padded index) in the chosen directory.

### Notes

- `target` accepts either a ritual YAML path or an `app:ritual` alias (requires the App Pack to be installed).
- Errors halt execution with a non‑zero exit; successful envelopes are still written for prior items when `--save` is used.
