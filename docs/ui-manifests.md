# UI Manifests for App Packs

UI Manifests enable App Pack authors to define custom, manifest-driven cards for displaying ritual run results in the Operate UI. Cards are automatically rendered based on the ritual being executed, providing a tailored viewing experience without requiring custom front-end code for each app.

## Overview

When a ritual run completes, the Operate UI:
1. Fetches matching card definitions from installed App Packs
2. Renders cards based on their `kind` and `config`
3. Displays cards in the run detail page and graph viewer
4. Extracts data from ritual events using JSON path notation

## Schema Version

Current schema: `ui-manifest.v1.schema.json`

Location: `contracts/schemas/ui-manifest.v1.schema.json`

## Card Types

The UI manifest supports four card types, each designed for specific data visualization needs.

### 1. result-envelope

Displays ritual execution results with status badges, timing information, and optional markdown content.

**Use cases:**
- Show success/failure status with visual badges
- Display execution duration and timestamps
- Render summary markdown from ritual outputs

**Configuration:**

```yaml
ui:
  cards:
    - id: my-result-card
      kind: result-envelope
      title: Execution Result
      description: Shows ritual execution outcome
      match:
        rituals:
          - my-ritual-name
      config:
        statusPath: result.success         # Required: Path to boolean/string status
        durationPath: duration              # Optional: Path to duration in milliseconds
        markdownPath: result.summary        # Optional: Path to markdown content
        showTimestamp: true                 # Optional: Show completion timestamp (default: true)
```

**Data extraction:**
- `statusPath`: Extracts from `ritual.completed:v1` event's `outputs` field
- Boolean values: `true` → "Success", `false` → "Failed"
- String values: Displayed as-is
- Default: `result.success`

**Output:**
- Status badge (color-coded: green for success, red for failure)
- Duration (formatted as ms, seconds, or minutes)
- Completion timestamp (ISO 8601 format)
- Markdown content (HTML-escaped)

---

### 2. fields-table

Displays structured data as a table with configurable field formatting.

**Use cases:**
- Show key-value pairs from ritual outputs
- Display structured metadata with labels
- Format values based on type (code, badge, timestamp, duration)

**Configuration:**

```yaml
ui:
  cards:
    - id: my-fields-table
      kind: fields-table
      title: Ritual Outputs
      description: Key information from the ritual execution
      match:
        rituals:
          - my-ritual-name
      config:
        fields:
          - label: Status
            path: result.success
            format: badge               # Options: text, code, badge, timestamp, duration
          - label: Message
            path: result.data.message
            format: text
          - label: Duration
            path: duration
            format: duration
          - label: Timestamp
            path: result.data.timestamp
            format: timestamp
```

**Config is required** for fields-table cards. Must include a `fields` array.

**Field formats:**
- `text`: Plain text (default)
- `code`: Monospace code styling
- `badge`: Status badge with color coding
- `timestamp`: Formatted timestamp display
- `duration`: Formatted duration (ms/s/m)

**Output:**
- Clean two-column table
- Left column: Field labels (bold)
- Right column: Formatted values
- Missing values: Displayed as "—"

---

### 3. markdown-view

Displays long-form markdown or text content with optional scrolling.

**Use cases:**
- Show detailed logs or output
- Display formatted documentation
- Render multi-line text content

**Configuration:**

```yaml
ui:
  cards:
    - id: my-markdown-view
      kind: markdown-view
      title: Ritual Logs
      description: Detailed execution logs
      match:
        rituals:
          - my-ritual-name
      config:
        contentPath: result.logs        # Required: Path to markdown content
        maxHeight: 400px                # Optional: Max height with scrolling
```

**Config is required** for markdown-view cards. Must include `contentPath`.

**Output:**
- Monospace font for code-like content
- Scrollable container if `maxHeight` is set
- HTML-escaped content (prevents XSS)
- Preserves newlines and formatting

---

### 4. json-viewer

Displays JSON data in a formatted, collapsible view.

**Use cases:**
- Inspect full ritual outputs
- Debug complex nested structures
- Explore arbitrary JSON data

**Configuration:**

```yaml
ui:
  cards:
    - id: my-json-viewer
      kind: json-viewer
      title: Full Output
      description: Complete ritual output as JSON
      match:
        rituals:
          - my-ritual-name
      config:                           # Config is optional for json-viewer
        rootPath: result.data           # Optional: Root path for JSON subset
        expandDepth: 2                  # Optional: Initial expansion depth (default: 2)
```

**Config is optional** for json-viewer cards.

**Output:**
- Pretty-printed JSON with indentation
- Syntax highlighting
- Scrollable container
- Displays entire `outputs` field by default, or subset if `rootPath` is specified

---

## JSON Path Extraction

All card types extract data from the `ritual.completed:v1` event's `outputs` field using dot-notation JSON paths.

**Syntax:**
- Dot notation: `result.data.message`
- Array indexing: `diagnostics[0].level`
- Nested access: `result.data.nested[1].field`

**Examples:**

```json
{
  "outputs": {
    "result": {
      "success": true,
      "data": {
        "message": "Hello World",
        "timestamp": "2025-11-01T00:00:00Z"
      }
    },
    "diagnostics": [
      { "level": "info", "message": "Starting" },
      { "level": "info", "message": "Completed" }
    ]
  }
}
```

Paths:
- `result.success` → `true`
- `result.data.message` → `"Hello World"`
- `diagnostics[0].level` → `"info"`
- `diagnostics[1].message` → `"Completed"`

**Missing paths:**
- Return `null` or empty value
- Displayed as "—" in fields-table
- Displayed as "*No content available*" in markdown-view

---

## Match Rules

Cards are matched to ritual runs using the `match` section:

```yaml
match:
  rituals:
    - ritual-one
    - ritual-two
  tags:                 # Future: tag-based matching
    - environment:prod
```

**Matching logic:**
- Card renders if ritual name is in the `rituals` list
- Multiple cards can match the same ritual
- Cards render in the order defined in the manifest

---

## Complete Example

```yaml
apiVersion: demon.io/v1
kind: AppPack
metadata:
  name: hello-world
  version: 1.0.0

rituals:
  - name: hello
    displayName: Hello World Ritual
    steps:
      - capsule: hello

ui:
  cards:
    # Result summary card
    - id: hello-result
      kind: result-envelope
      title: Execution Summary
      description: Shows the execution outcome and timing
      match:
        rituals:
          - hello
      config:
        statusPath: result.success
        durationPath: duration
        showTimestamp: true

    # Detailed fields card
    - id: hello-fields
      kind: fields-table
      title: Output Details
      description: Structured output fields
      match:
        rituals:
          - hello
      config:
        fields:
          - label: Message
            path: result.data.message
            format: code
          - label: Timestamp
            path: result.data.timestamp
            format: timestamp

    # Full JSON inspection
    - id: hello-json
      kind: json-viewer
      title: Complete Output
      description: Full ritual output for debugging
      match:
        rituals:
          - hello
      config:
        expandDepth: 3
```

---

## Card Rendering Lifecycle

1. **Run Execution**: Ritual completes and publishes `ritual.completed:v1` event with `outputs`
2. **Route Handler**: `get_run_html_tenant()` fetches run details from JetStream
3. **Card Matching**: App Pack registry finds cards where `match.rituals` includes the ritual ID
4. **Rendering**: Each card is rendered using its specific renderer:
   - `render_result_envelope()`
   - `render_fields_table()`
   - `render_markdown_view()`
   - `render_json_viewer()`
5. **Template Display**: Rendered HTML is inserted into `run_detail.html` template
6. **Error Handling**: Card rendering errors are logged; failed cards don't crash the page

---

## Styling and CSS

Cards automatically receive CSS styling:

**Card container:**
- `.app-pack-card`: Main card wrapper
- `.app-pack-card-header`: Title and badge area
- `.app-pack-card-title`: Card title text
- `.card-kind-badge`: Card type badge (result-envelope, fields-table, etc.)
- `.app-pack-card-description`: Optional description text
- `.app-pack-card-content`: Rendered card HTML

**Card-specific styles:**
- `.result-envelope-card`: Result envelope styling
- `.fields-table`: Two-column table layout
- `.markdown-view-card`: Markdown container
- `.json-viewer-card`: JSON display area
- `.status-badge`: Status indicator with color coding
  - `.status-success`: Green success badge
  - `.status-failed`: Red failure badge

**Customization:**
Cards use the Operate UI's existing CSS variables and design system. Custom styling can be added via `static/` assets in App Packs (future enhancement).

---

## Graph Viewer Integration

Cards are accessible from the graph viewer:

1. **Run Detail → Graph**: Click "View in Graph" button in App Pack Cards section
2. **Graph URL**: Navigate to `/graph?runId=<run-id>`
3. **Side Panel**: Cards display in a dedicated panel below the graph
4. **Navigation**: "View Run Detail" link returns to full run detail page

**Workflow:**
- Explore ritual run in detail view
- Click "View in Graph" to see execution in graph context
- View cards alongside graph visualization
- Navigate back to run detail for full view

---

## Error Handling

**Card rendering failures:**
- Logged as warnings (visible in server logs)
- Failed cards are skipped (don't crash page)
- Other cards continue rendering normally

**Missing data:**
- Missing JSON paths display default values
- Missing `ritual.completed` event: cards section doesn't render
- No matching cards: cards section hidden

**Validation:**
- Manifests are validated against `ui-manifest.v1.schema.json`
- Invalid configs are rejected at install time (via `demonctl app install`)
- Runtime validation ensures required fields are present

---

## Testing

**Unit tests:**
```bash
cargo test -p operate-ui
```

**Playwright UI tests:**
```bash
cd operate-ui/playwright
npm install
npm test tests/app_pack_cards.spec.ts
```

**Manual testing:**
1. Install app-pack-sample: `demonctl app install examples/app-pack-sample`
2. Execute hello ritual: `demonctl run examples/rituals/hello.yaml`
3. View run in Operate UI: `http://localhost:3000/runs/<run-id>`
4. Verify cards display correctly

---

## Best Practices

**Card design:**
- Use `result-envelope` for high-level status summaries
- Use `fields-table` for structured key-value data (limit to 5-10 fields)
- Use `markdown-view` for logs or long-form text (set `maxHeight` for long content)
- Use `json-viewer` for debugging and technical inspection

**Performance:**
- Limit cards to 3-5 per ritual for optimal UX
- Use `json-viewer` sparingly (can be slow with large outputs)
- Prefer `fields-table` over `json-viewer` for specific fields

**Maintainability:**
- Use descriptive card IDs and titles
- Include helpful descriptions for each card
- Document expected output structure in App Pack README
- Version manifests with the App Pack

**Security:**
- All HTML is automatically escaped (XSS prevention)
- JSON paths are sanitized before extraction
- No client-side JavaScript execution in card content

---

## Troubleshooting

**Cards not appearing:**
1. Verify App Pack is installed: `demonctl app list`
2. Check ritual name matches `match.rituals` exactly
3. Confirm `ritual.completed:v1` event was published
4. Check Operate UI logs for rendering errors

**Missing data in cards:**
1. Verify JSON path in config (e.g., `result.data.message`)
2. Inspect `ritual.completed:v1` event's `outputs` field
3. Use `json-viewer` card to explore full output structure
4. Check for typos in field names (paths are case-sensitive)

**Cards show wrong data:**
1. Verify `statusPath`, `contentPath`, or field `path` values
2. Ensure ritual outputs match expected structure
3. Check for incorrect array indexing (e.g., `items[0]` vs `items[1]`)

**Styling issues:**
1. Ensure Operate UI CSS files are up to date
2. Check for custom CSS conflicts in `static/` directory
3. Verify card HTML structure matches expected classes

---

## Schema Reference

See `contracts/schemas/ui-manifest.v1.schema.json` for the complete JSON Schema definition.

**Key schema features:**
- `allOf` constraints ensure `kind` matches `config` shape
- Required `config` for `fields-table` and `markdown-view`
- Optional `config` for `result-envelope` and `json-viewer`
- Strict validation prevents mismatched kind/config combinations

---

## Future Enhancements

- **Tag-based matching**: Match cards based on ritual tags, not just names
- **Custom CSS**: App Packs can bundle custom stylesheets
- **Interactive cards**: Client-side JavaScript for interactive visualizations
- **Card templates**: Reusable card definitions across multiple rituals
- **Conditional rendering**: Show/hide cards based on output values

---

## Related Documentation

- [App Packs Guide](./app-packs.md) - Complete App Pack authoring guide
- [Operate UI](./operate-ui/README.md) - Operate UI overview and usage
- [Ritual Event Schemas](../contracts/schemas/) - Event contract definitions
