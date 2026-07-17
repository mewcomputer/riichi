use riichi_persistence::DocumentReferenceInput;
use riichi_persistence::Error;
use serde_json::Value;
use std::fmt::Write;

pub fn tiptap_plain_text(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let mut text = object
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if object.get("type").and_then(Value::as_str) == Some("riichiLink") {
                text = object
                    .get("attrs")
                    .and_then(|attrs| attrs.get("label"))
                    .and_then(Value::as_str)
                    .unwrap_or("linked resource")
                    .to_owned();
            }
            if let Some(content) = object.get("content").and_then(Value::as_array) {
                for child in content {
                    let child_text = tiptap_plain_text(child);
                    if !text.is_empty() && !child_text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&child_text);
                }
            }
            text
        }
        Value::Array(values) => values
            .iter()
            .map(tiptap_plain_text)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub fn tiptap_sanitized_html(value: &Value) -> Result<String, Error> {
    let mut html = String::new();
    render_node(value, &mut html)?;
    if html.len() > 500_000 {
        return Err(Error::InvalidDocument(
            "document HTML is too large".to_owned(),
        ));
    }
    Ok(html)
}

pub fn tiptap_document_references(value: &Value) -> Vec<DocumentReferenceInput> {
    let mut references = Vec::new();
    collect_document_references(value, &mut Vec::new(), &mut references);
    references.sort_by(|left, right| {
        (
            left.source_block_id.as_str(),
            left.resource_kind.as_str(),
            left.resource_id,
        )
            .cmp(&(
                right.source_block_id.as_str(),
                right.resource_kind.as_str(),
                right.resource_id,
            ))
    });
    references.dedup_by(|left, right| {
        left.source_block_id == right.source_block_id
            && left.resource_kind == right.resource_kind
            && left.resource_id == right.resource_id
            && left.reference_kind == right.reference_kind
    });
    references
}

fn collect_document_references(
    value: &Value,
    path: &mut Vec<usize>,
    references: &mut Vec<DocumentReferenceInput>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    if object.get("type").and_then(Value::as_str) == Some("text")
        && let Some(marks) = object.get("marks").and_then(Value::as_array)
    {
        for mark in marks {
            let Some(attrs) = mark.get("attrs") else {
                continue;
            };
            let Some(kind) = attrs
                .get("resourceKind")
                .or_else(|| attrs.get("resourceType"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            let Some(id) = attrs
                .get("resourceId")
                .and_then(Value::as_str)
                .and_then(|value| value.parse().ok())
            else {
                continue;
            };
            let source_block_id = attrs
                .get("sourceBlockId")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    format!(
                        "block-{}",
                        path.iter()
                            .map(usize::to_string)
                            .collect::<Vec<_>>()
                            .join("-")
                    )
                });
            references.push(DocumentReferenceInput {
                source_block_id,
                resource_kind: kind.to_owned(),
                resource_id: id,
                reference_kind: "inline".to_owned(),
            });
        }
    }
    if let Some(children) = object.get("content").and_then(Value::as_array) {
        for (index, child) in children.iter().enumerate() {
            path.push(index);
            collect_document_references(child, path, references);
            path.pop();
        }
    }
}

fn render_node(value: &Value, output: &mut String) -> Result<(), Error> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::InvalidDocument("document node must be an object".to_owned()))?;
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::InvalidDocument("document node is missing its type".to_owned()))?;
    match node_type {
        "doc" => render_children(object, output),
        "paragraph" => render_wrapped(object, output, "p"),
        "heading" => {
            let level = object
                .get("attrs")
                .and_then(|attrs| attrs.get("level"))
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 6);
            render_children_with_tags(object, output, &format!("h{level}"), &format!("h{level}"))
        }
        "bulletList" => render_wrapped(object, output, "ul"),
        "orderedList" => render_wrapped(object, output, "ol"),
        "listItem" => render_wrapped(object, output, "li"),
        "taskList" => {
            output.push_str("<ul data-type=\"taskList\">");
            render_children(object, output)?;
            output.push_str("</ul>");
            Ok(())
        }
        "taskItem" => {
            let checked = object
                .get("attrs")
                .and_then(|attrs| attrs.get("checked"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            write!(
                output,
                "<li data-type=\"taskItem\" data-checked=\"{checked}\">"
            )
            .map_err(|_| Error::InvalidDocument("failed to render task item".to_owned()))?;
            render_children(object, output)?;
            output.push_str("</li>");
            Ok(())
        }
        "blockquote" => render_wrapped(object, output, "blockquote"),
        "callout" => {
            let kind = object
                .get("attrs")
                .and_then(|attrs| attrs.get("kind"))
                .and_then(Value::as_str)
                .filter(|kind| matches!(*kind, "info" | "warning" | "success" | "danger"))
                .ok_or_else(|| Error::InvalidDocument("callout kind is invalid".to_owned()))?;
            write!(output, "<aside data-callout=\"{}\">", escape_html(kind))
                .map_err(|_| Error::InvalidDocument("failed to render callout".to_owned()))?;
            render_children(object, output)?;
            output.push_str("</aside>");
            Ok(())
        }
        "codeBlock" => {
            output.push_str("<pre><code>");
            render_text_children(object, output, false)?;
            output.push_str("</code></pre>");
            Ok(())
        }
        "hardBreak" => {
            output.push_str("<br>");
            Ok(())
        }
        "horizontalRule" => {
            output.push_str("<hr>");
            Ok(())
        }
        "text" => render_text_node(object, output, true),
        "mention" => {
            let label = object
                .get("attrs")
                .and_then(|attrs| attrs.get("label").or_else(|| attrs.get("id")))
                .and_then(Value::as_str)
                .unwrap_or("mention");
            write!(
                output,
                "<span data-mention>\u{40}{}</span>",
                escape_html(label)
            )
            .map_err(|_| Error::InvalidDocument("failed to render mention".to_owned()))
        }
        "riichiLink" => {
            let attrs = object
                .get("attrs")
                .and_then(Value::as_object)
                .ok_or_else(|| Error::InvalidDocument("riichi link is missing attrs".to_owned()))?;
            let kind = attrs
                .get("resourceType")
                .and_then(Value::as_str)
                .filter(|kind| matches!(*kind, "issue" | "team" | "project" | "document"))
                .ok_or_else(|| {
                    Error::InvalidDocument("riichi link has an invalid resource type".to_owned())
                })?;
            let resource_id = attrs
                .get("resourceId")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    Error::InvalidDocument("riichi link is missing its resource ID".to_owned())
                })?;
            let label = attrs
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("linked resource");
            write!(
                output,
                "<a data-riichi-link data-resource-kind=\"{}\" data-resource-id=\"{}\" href=\"#/{}/{}\">{}</a>",
                escape_html(kind),
                escape_html(resource_id),
                escape_html(kind),
                escape_html(resource_id),
                escape_html(label)
            )
            .map_err(|_| Error::InvalidDocument("failed to render riichi link".to_owned()))
        }
        "image" => {
            let attachment_id = object
                .get("attrs")
                .and_then(|attrs| attrs.get("attachmentId"))
                .and_then(Value::as_str)
                .unwrap_or("attachment");
            write!(
                output,
                "<span data-attachment=\"{}\">[attachment]</span>",
                escape_html(attachment_id)
            )
            .map_err(|_| Error::InvalidDocument("failed to render attachment".to_owned()))
        }
        _ => Err(Error::InvalidDocument(format!(
            "unsupported document node: {node_type}"
        ))),
    }
}

fn render_children(
    object: &serde_json::Map<String, Value>,
    output: &mut String,
) -> Result<(), Error> {
    if let Some(children) = object.get("content").and_then(Value::as_array) {
        for child in children {
            render_node(child, output)?;
        }
    }
    Ok(())
}

fn render_wrapped(
    object: &serde_json::Map<String, Value>,
    output: &mut String,
    tag: &str,
) -> Result<(), Error> {
    render_children_with_tags(object, output, tag, tag)
}

fn render_children_with_tags(
    object: &serde_json::Map<String, Value>,
    output: &mut String,
    open_tag: &str,
    close_tag: &str,
) -> Result<(), Error> {
    write!(output, "<{open_tag}>")
        .map_err(|_| Error::InvalidDocument("failed to render document".to_owned()))?;
    render_children(object, output)?;
    write!(output, "</{close_tag}>")
        .map_err(|_| Error::InvalidDocument("failed to render document".to_owned()))
}

fn render_text_children(
    object: &serde_json::Map<String, Value>,
    output: &mut String,
    with_marks: bool,
) -> Result<(), Error> {
    if let Some(children) = object.get("content").and_then(Value::as_array) {
        for child in children {
            let child_object = child.as_object().ok_or_else(|| {
                Error::InvalidDocument("document child must be an object".to_owned())
            })?;
            if child_object.get("type").and_then(Value::as_str) == Some("text") {
                render_text_node(child_object, output, with_marks)?;
            } else {
                render_node(child, output)?;
            }
        }
    }
    Ok(())
}

fn render_text_node(
    object: &serde_json::Map<String, Value>,
    output: &mut String,
    with_marks: bool,
) -> Result<(), Error> {
    let mut text = escape_html(
        object
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if with_marks {
        let marks = object.get("marks").and_then(Value::as_array);
        if marks.is_some_and(|marks| {
            marks
                .iter()
                .any(|mark| mark.get("type").and_then(Value::as_str) == Some("link"))
        }) {
            let href = marks
                .and_then(|marks| {
                    marks
                        .iter()
                        .find(|mark| mark.get("type").and_then(Value::as_str) == Some("link"))
                })
                .and_then(|mark| mark.get("attrs"))
                .and_then(|attrs| attrs.get("href"))
                .and_then(Value::as_str)
                .filter(|href| {
                    href.starts_with("https://")
                        || href.starts_with("http://")
                        || href.starts_with('/')
                })
                .unwrap_or("#");
            text = format!("<a href=\"{}\">{text}</a>", escape_html(href));
        }
        if let Some(marks) = marks {
            for mark in marks.iter().rev() {
                match mark.get("type").and_then(Value::as_str) {
                    Some("bold") => text = format!("<strong>{text}</strong>"),
                    Some("italic") => text = format!("<em>{text}</em>"),
                    Some("strike") => text = format!("<s>{text}</s>"),
                    Some("code") => text = format!("<code>{text}</code>"),
                    Some("link") | None => {}
                    Some(other) => {
                        return Err(Error::InvalidDocument(format!(
                            "unsupported text mark: {other}"
                        )));
                    }
                }
            }
        }
    }
    output.push_str(&text);
    Ok(())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_supported_tiptap_content_without_trusting_html() {
        let content = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "<script>alert(1)</script>",
                    "marks": [{"type": "bold"}]
                }]
            }]
        });
        let html = tiptap_sanitized_html(&content).unwrap();
        assert_eq!(
            html,
            "<p><strong>&lt;script&gt;alert(1)&lt;/script&gt;</strong></p>"
        );
    }

    #[test]
    fn rejects_unknown_nodes_and_unsafe_links() {
        let content = json!({
            "type": "doc",
            "content": [{"type": "unknown"}]
        });
        assert!(matches!(
            tiptap_sanitized_html(&content),
            Err(Error::InvalidDocument(_))
        ));

        let link = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [{
                "type": "text", "text": "click",
                "marks": [{"type": "link", "attrs": {"href": "javascript:alert(1)"}}]
            }]}]
        });
        assert_eq!(
            tiptap_sanitized_html(&link).unwrap(),
            "<p><a href=\"#\">click</a></p>"
        );
    }

    #[test]
    fn renders_riichi_links_with_stable_resource_attributes() {
        let content = json!({
            "type": "doc",
            "content": [{
                "type": "riichiLink",
                "attrs": {
                    "resourceType": "issue",
                    "resourceId": "019f5942-ba13-7153-be85-59ee672d6b6c",
                    "label": "RII-42"
                }
            }]
        });
        let html = tiptap_sanitized_html(&content).unwrap();
        assert!(html.contains("data-riichi-link"));
        assert!(html.contains("RII-42"));
        assert_eq!(tiptap_plain_text(&content), "RII-42");
    }

    #[test]
    fn extracts_deduplicated_resource_references_from_link_marks() {
        let resource_id = "019f5942-ba13-7153-be85-59ee672d6b6c";
        let content = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [
                {"type": "text", "text": "RII-42", "marks": [{"type": "link", "attrs": {
                    "resourceKind": "issue", "resourceId": resource_id, "sourceBlockId": "block-1"
                }}]},
                {"type": "text", "text": " again", "marks": [{"type": "link", "attrs": {
                    "resourceKind": "issue", "resourceId": resource_id, "sourceBlockId": "block-1"
                }}]}
            ]}]
        });
        let references = tiptap_document_references(&content);
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].resource_kind, "issue");
        assert_eq!(references[0].source_block_id, "block-1");
    }
}
