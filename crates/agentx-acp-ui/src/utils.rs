use agent_client_protocol::{self as acp, ToolKind};
use serde_json::Value;

pub fn truncate_lines(text: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return text.to_string();
    }

    let mut result = String::new();
    for (index, line) in text.lines().take(max_lines).enumerate() {
        if index > 0 {
            result.push('\n');
        }
        result.push_str(line);
    }

    result
}

pub fn extract_terminal_output(terminal: &acp::Terminal) -> Option<String> {
    let meta = terminal.meta.as_ref()?;
    extract_terminal_output_from_meta(meta)
}

pub fn extract_xml_content(text: &str, tool_kind: &ToolKind) -> String {
    let should_extract = matches!(
        tool_kind,
        ToolKind::Execute | ToolKind::Other | ToolKind::Read
    );

    let mut filtered_text = strip_system_reminder_blocks(text);
    if let Some(start) = text.find("<system-reminder>") {
        let prefix = &text[..start];
        if !prefix.trim().is_empty() {
            filtered_text = prefix.to_string();
        }
    }

    if !should_extract {
        return strip_code_fences(&filtered_text);
    }

    let extracted = extract_tagged_text(&filtered_text);
    if extracted.trim().is_empty() {
        strip_code_fences(&filtered_text)
    } else {
        extracted
    }
}

fn strip_code_fences(text: &str) -> String {
    text.trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string()
}

fn strip_system_reminder_blocks(text: &str) -> String {
    let open_tag = "<system-reminder>";
    let close_tag = "</system-reminder>";
    let mut remainder = text;
    let mut output = String::with_capacity(text.len());

    while let Some(start) = remainder.find(open_tag) {
        output.push_str(&remainder[..start]);
        let after_open = &remainder[start + open_tag.len()..];
        if let Some(end) = after_open.find(close_tag) {
            remainder = &after_open[end + close_tag.len()..];
        } else {
            remainder = after_open;
            break;
        }
    }

    output.push_str(remainder);
    output
}

fn extract_tagged_text(text: &str) -> String {
    let mut result = String::new();
    let mut cursor = 0;

    while let Some(start) = text[cursor..].find('<') {
        let tag_start = cursor + start;
        let name_start = tag_start + 1;

        if text.get(name_start..name_start + 1) == Some("/") {
            cursor = name_start + 1;
            continue;
        }

        let mut name_end = name_start;
        for ch in text[name_start..].chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                name_end += ch.len_utf8();
            } else {
                break;
            }
        }

        if name_end == name_start {
            cursor = name_start;
            continue;
        }

        let tag_name = &text[name_start..name_end];
        if tag_name.eq_ignore_ascii_case("system-reminder") {
            cursor = name_end;
            continue;
        }

        let open_end = match text[name_end..].find('>') {
            Some(offset) => name_end + offset,
            None => break,
        };

        let closing_tag = format!("</{}>", tag_name);
        let after_open = open_end + 1;
        let close_start = match text[after_open..].find(&closing_tag) {
            Some(offset) => after_open + offset,
            None => {
                cursor = after_open;
                continue;
            }
        };

        let content = text[after_open..close_start].trim();
        if !content.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(content);
        }

        cursor = close_start + closing_tag.len();
    }

    result
}

fn extract_terminal_output_from_meta(meta: &serde_json::Map<String, Value>) -> Option<String> {
    let direct = meta
        .get("output")
        .or_else(|| meta.get("text"))
        .or_else(|| meta.get("content"));
    if let Some(value) = direct {
        return value_to_string(value);
    }

    let nested = meta
        .get("terminal_output")
        .or_else(|| meta.get("terminalOutput"));
    if let Some(Value::Object(obj)) = nested {
        return extract_terminal_output_from_meta(obj);
    }

    None
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let mut lines = Vec::new();
            for item in items {
                if let Some(text) = value_to_string(item) {
                    lines.push(text);
                }
            }
            if lines.is_empty() {
                None
            } else {
                Some(lines.join("\n"))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_lines_limits_output() {
        let text = "line1\nline2\nline3";
        assert_eq!(truncate_lines(text, 2), "line1\nline2");
        assert_eq!(truncate_lines(text, 0), text);
    }

    #[test]
    fn extract_xml_content_strips_code_fences() {
        let text = "```\ncontent\n```";
        let cleaned = extract_xml_content(text, &ToolKind::Search);
        assert_eq!(cleaned, "content");
    }

    #[test]
    fn extract_terminal_output_reads_nested_meta() {
        let terminal = acp::Terminal::new("term-1").meta(serde_json::json!({
            "terminal_output": {
                "output": ["line1", "line2"]
            }
        }));
        let output = extract_terminal_output(&terminal).unwrap();
        assert_eq!(output, "line1\nline2");
    }
}
