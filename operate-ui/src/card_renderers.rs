use crate::app_packs::CardDefinition;
use crate::jetstream::RunDetail;
use anyhow::Result;
use serde_json::Value;

/// Rendered card data ready for template insertion
#[derive(Debug, Clone, serde::Serialize)]
pub struct RenderedCard {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub description: Option<String>,
    pub html: String,
}

/// Render a card based on its kind
pub fn render_card(card: &CardDefinition, run: &RunDetail) -> Result<RenderedCard> {
    let html = match card.kind.as_str() {
        "result-envelope" => render_result_envelope(card, run)?,
        "fields-table" => render_fields_table(card, run)?,
        "markdown-view" => render_markdown_view(card, run)?,
        "json-viewer" => render_json_viewer(card, run)?,
        unknown => {
            return Err(anyhow::anyhow!(
                "Unknown card kind '{}' for card '{}'",
                unknown,
                card.id
            ))
        }
    };

    Ok(RenderedCard {
        id: card.id.clone(),
        kind: card.kind.clone(),
        title: card.title.clone().unwrap_or_else(|| card.id.clone()),
        description: card.description.clone(),
        html,
    })
}

/// Render a result-envelope card
fn render_result_envelope(card: &CardDefinition, run: &RunDetail) -> Result<String> {
    let config = card.get_config().unwrap_or(&Value::Null);

    // Extract config paths
    let status_path = config
        .get("statusPath")
        .and_then(|v| v.as_str())
        .unwrap_or("result.success");
    let duration_path = config.get("durationPath").and_then(|v| v.as_str());
    let markdown_path = config.get("markdownPath").and_then(|v| v.as_str());
    let show_timestamp = config
        .get("showTimestamp")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Find the ritual.completed event to extract outputs
    let completed_event = run.events.iter().find(|e| e.event == "ritual.completed:v1");

    let outputs = completed_event
        .and_then(|e| e.extra.get("outputs"))
        .unwrap_or(&Value::Null);

    // Extract status
    let status_value = extract_json_path(outputs, status_path);
    let status = format_status(&status_value);

    // Extract duration
    let duration = duration_path
        .and_then(|path| extract_json_path(outputs, path))
        .and_then(|v| v.as_f64())
        .map(|ms| format_duration(ms));

    // Extract markdown content
    let markdown_content = markdown_path
        .and_then(|path| extract_json_path(outputs, path))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Build HTML
    let mut html = String::new();
    html.push_str("<div class=\"result-envelope-card\">");

    // Status badge
    html.push_str(&format!(
        "<div class=\"card-status\"><span class=\"status-badge status-{}\">{}</span></div>",
        status.to_lowercase(),
        status
    ));

    // Duration if available
    if let Some(dur) = duration {
        html.push_str(&format!(
            "<div class=\"card-duration\"><strong>Duration:</strong> {}</div>",
            dur
        ));
    }

    // Timestamp if requested
    if show_timestamp {
        if let Some(event) = completed_event {
            html.push_str(&format!(
                "<div class=\"card-timestamp\"><strong>Completed:</strong> {}</div>",
                event.ts.to_rfc3339()
            ));
        }
    }

    // Markdown content if available
    if let Some(md) = markdown_content {
        html.push_str("<div class=\"card-markdown\">");
        // Basic markdown rendering (can be enhanced with pulldown-cmark)
        html.push_str(&escape_html(&md));
        html.push_str("</div>");
    }

    html.push_str("</div>");
    Ok(html)
}

/// Render a fields-table card
fn render_fields_table(card: &CardDefinition, run: &RunDetail) -> Result<String> {
    let config = card.get_config().ok_or_else(|| {
        anyhow::anyhow!(
            "fields-table card '{}' requires config with fields array",
            card.id
        )
    })?;

    let fields = config
        .get("fields")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("fields-table config must have 'fields' array"))?;

    // Find ritual.completed event
    let completed_event = run.events.iter().find(|e| e.event == "ritual.completed:v1");

    let outputs = completed_event
        .and_then(|e| e.extra.get("outputs"))
        .unwrap_or(&Value::Null);

    let mut html = String::new();
    html.push_str("<div class=\"fields-table-card\"><table class=\"fields-table\">");

    for field in fields {
        let label = field
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let path = field.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let format = field
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        let value = extract_json_path(outputs, path);
        let formatted_value = format_field_value(&value, format);

        html.push_str(&format!(
            "<tr><td class=\"field-label\"><strong>{}</strong></td><td class=\"field-value\">{}</td></tr>",
            escape_html(label),
            formatted_value
        ));
    }

    html.push_str("</table></div>");
    Ok(html)
}

/// Render a markdown-view card
fn render_markdown_view(card: &CardDefinition, run: &RunDetail) -> Result<String> {
    let config = card.get_config().ok_or_else(|| {
        anyhow::anyhow!(
            "markdown-view card '{}' requires config with contentPath",
            card.id
        )
    })?;

    let content_path = config
        .get("contentPath")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("markdown-view config must have 'contentPath'"))?;

    let max_height = config.get("maxHeight").and_then(|v| v.as_str());

    // Find ritual.completed event
    let completed_event = run.events.iter().find(|e| e.event == "ritual.completed:v1");

    let outputs = completed_event
        .and_then(|e| e.extra.get("outputs"))
        .unwrap_or(&Value::Null);

    let markdown_content = extract_json_path(outputs, content_path)
        .and_then(|v| v.as_str())
        .unwrap_or("*No content available*");

    let style = max_height
        .map(|h| format!(" style=\"max-height: {}; overflow-y: auto;\"", h))
        .unwrap_or_default();

    let html = format!(
        "<div class=\"markdown-view-card\"{}>
            <div class=\"markdown-content\">{}</div>
        </div>",
        style,
        escape_html(markdown_content)
    );

    Ok(html)
}

/// Render a json-viewer card
fn render_json_viewer(card: &CardDefinition, run: &RunDetail) -> Result<String> {
    let config = card.get_config().unwrap_or(&Value::Null);

    let root_path = config.get("rootPath").and_then(|v| v.as_str());
    let expand_depth = config
        .get("expandDepth")
        .and_then(|v| v.as_u64())
        .unwrap_or(2) as usize;

    // Find ritual.completed event
    let completed_event = run.events.iter().find(|e| e.event == "ritual.completed:v1");

    let outputs = completed_event
        .and_then(|e| e.extra.get("outputs"))
        .unwrap_or(&Value::Null);

    let json_data = root_path
        .and_then(|path| extract_json_path(outputs, path))
        .unwrap_or(outputs);

    // Pretty print JSON with limited depth expansion
    let json_string = serde_json::to_string_pretty(json_data).unwrap_or_else(|_| "{}".to_string());

    let html = format!(
        "<div class=\"json-viewer-card\" data-expand-depth=\"{}\">
            <pre class=\"json-content\"><code>{}</code></pre>
        </div>",
        expand_depth,
        escape_html(&json_string)
    );

    Ok(html)
}

// Helper functions

/// Extract a value from JSON using a dot-notation path
fn extract_json_path<'a>(data: &'a Value, path: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = data;

    for part in parts {
        // Handle array indexing (e.g., "diagnostics[0]")
        if let Some(idx_start) = part.find('[') {
            if let Some(idx_end) = part.find(']') {
                let field = &part[..idx_start];
                let index_str = &part[idx_start + 1..idx_end];

                current = current.get(field)?;

                if let Ok(index) = index_str.parse::<usize>() {
                    current = current.get(index)?;
                } else {
                    return None;
                }
                continue;
            }
        }

        current = current.get(part)?;
    }

    Some(current)
}

/// Format a status value into a human-readable string
fn format_status(value: &Option<&Value>) -> String {
    match value {
        Some(Value::Bool(true)) => "Success".to_string(),
        Some(Value::Bool(false)) => "Failed".to_string(),
        Some(Value::String(s)) => s.clone(),
        _ => "Unknown".to_string(),
    }
}

/// Format duration in milliseconds
fn format_duration(ms: f64) -> String {
    if ms < 1000.0 {
        format!("{:.2}ms", ms)
    } else if ms < 60000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else {
        let minutes = (ms / 60000.0).floor();
        let seconds = (ms % 60000.0) / 1000.0;
        format!("{}m {:.2}s", minutes, seconds)
    }
}

/// Format a field value based on its format type
fn format_field_value(value: &Option<&Value>, format: &str) -> String {
    match value {
        None => "<em>—</em>".to_string(),
        Some(v) => match format {
            "code" => format!("<code>{}</code>", escape_html(&v.to_string())),
            "badge" => {
                let status = format_status(&Some(v));
                format!(
                    "<span class=\"status-badge status-{}\">{}</span>",
                    status.to_lowercase(),
                    status
                )
            }
            "timestamp" => v
                .as_str()
                .map(|s| format!("<time>{}</time>", escape_html(s)))
                .unwrap_or_else(|| v.to_string()),
            "duration" => v
                .as_f64()
                .map(format_duration)
                .unwrap_or_else(|| v.to_string()),
            _ => escape_html(&v.to_string()),
        },
    }
}

/// Basic HTML escaping
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_path() {
        let data = serde_json::json!({
            "result": {
                "success": true,
                "data": {
                    "message": "Hello"
                }
            },
            "diagnostics": [
                {"level": "info", "message": "Test"}
            ]
        });

        assert_eq!(
            extract_json_path(&data, "result.success"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            extract_json_path(&data, "result.data.message"),
            Some(&serde_json::json!("Hello"))
        );
        assert_eq!(
            extract_json_path(&data, "diagnostics[0].message"),
            Some(&serde_json::json!("Test"))
        );
        assert_eq!(extract_json_path(&data, "nonexistent"), None);
    }

    #[test]
    fn test_format_status() {
        assert_eq!(format_status(&Some(&serde_json::json!(true))), "Success");
        assert_eq!(format_status(&Some(&serde_json::json!(false))), "Failed");
        assert_eq!(
            format_status(&Some(&serde_json::json!("Pending"))),
            "Pending"
        );
        assert_eq!(format_status(&None), "Unknown");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(123.45), "123.45ms");
        assert_eq!(format_duration(1500.0), "1.50s");
        assert_eq!(format_duration(125000.0), "2m 5.00s");
    }
}
