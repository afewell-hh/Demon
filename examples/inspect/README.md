# demonctl inspect — Sample Outputs

This directory contains sample outputs from the `demonctl inspect` command for documentation and testing reference.

## Files

### Table Output (Plain Text)

- **sample_output_ok.txt** — System operating normally (OK status, steady recommendation)
- **sample_output_warn.txt** — Warning state with elevated metrics (scale up recommendation)
- **sample_output_error.txt** — Critical state with metrics exceeding error thresholds
- **sample_output_scale_down.txt** — Underutilized system (scale down recommendation)

### JSON Output

- **sample_output_ok.json** — JSON format for OK status
- **sample_output_warn.json** — JSON format for WARN status
- **sample_output_error.json** — JSON format for ERROR status

## Usage

These samples demonstrate:

1. **Status classification** — OK, WARN, ERROR based on threshold evaluation
2. **Scale recommendations** — Scale Up, Scale Down, Steady
3. **Metric formatting** — Latency units (ms vs seconds), error rate percentages
4. **Color coding** — (shown in terminal, not in plain text files)
5. **JSON schema compliance** — Machine-readable output structure

## Testing Against Live System

To capture your own output:

```bash
# Table output
demonctl inspect --graph > my_output.txt

# JSON output
demonctl inspect --graph --json > my_output.json

# With NO_COLOR for consistent formatting
NO_COLOR=1 demonctl inspect --graph > my_output_no_color.txt
```

## See Also

- [CLI Inspect Documentation](../../docs/cli-inspect.md)
- [Scale Feedback Telemetry](../../docs/scale-feedback.md)
