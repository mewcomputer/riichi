use chrono::{DateTime, Utc};
use loro::{
    Container, ExpandType, ExportMode, ID, ImportStatus, LoroDoc, LoroEncodeError, LoroList,
    LoroMap, LoroText, StyleConfig, StyleConfigMap, TextDelta, ToJson, ValueOrContainer,
    VersionVector,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{tiptap_plain_text, tiptap_sanitized_html};

const ROOT_CONTAINER: &str = "doc";
const ATTRIBUTES_KEY: &str = "attributes";
const CHILDREN_KEY: &str = "children";
const NODE_NAME_KEY: &str = "nodeName";

pub const DOCUMENT_SCHEMA_V1: i32 = 1;
pub const DOCUMENT_SCHEMA_V2: i32 = 2;
pub const CURRENT_DOCUMENT_SCHEMA_VERSION: i32 = DOCUMENT_SCHEMA_V2;

#[derive(Debug, Error)]
pub enum LoroDocumentError {
    #[error("document is invalid: {0}")]
    Invalid(String),

    #[error("failed to encode Loro document: {0}")]
    Encode(String),

    #[error("failed to import Loro document data: {0}")]
    Import(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoroFrontier {
    pub peer_id: u64,
    pub counter: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedLoroUpdate {
    pub update_id: Uuid,
    pub document_id: Uuid,
    pub principal_id: Uuid,
    pub source: String,
    pub payload: Vec<u8>,
    pub payload_sha256: String,
    pub previous_frontiers: Vec<LoroFrontier>,
    pub resulting_frontiers: Vec<LoroFrontier>,
    pub accepted_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LoroSnapshot {
    pub document_id: Uuid,
    pub revision: i64,
    pub schema_version: i32,
    pub frontiers: Vec<LoroFrontier>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct LoroUpdateResult {
    pub update_id: Uuid,
    pub document_id: Uuid,
    pub source: String,
    pub previous_frontiers: Vec<LoroFrontier>,
    pub resulting_frontiers: Vec<LoroFrontier>,
    pub accepted_at: DateTime<Utc>,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct LoroUpdateCommand {
    pub schema_version: i32,
    pub update_id: Uuid,
    pub idempotency_key: Option<String>,
    pub previous_frontiers: Vec<LoroFrontier>,
    pub payload: Vec<u8>,
    pub source: String,
}

#[derive(Debug)]
pub struct LoroDocument {
    document_id: Uuid,
    doc: LoroDoc,
}

impl LoroDocument {
    pub fn from_tiptap(document_id: Uuid, value: &Value) -> Result<Self, LoroDocumentError> {
        Self::from_tiptap_for_schema(document_id, value, CURRENT_DOCUMENT_SCHEMA_VERSION)
    }

    pub fn from_tiptap_for_schema(
        document_id: Uuid,
        value: &Value,
        schema_version: i32,
    ) -> Result<Self, LoroDocumentError> {
        validate_tiptap_for_schema(value, schema_version)?;

        let doc = configured_loro_doc();
        append_node(&doc.get_map(ROOT_CONTAINER), value)?;
        Ok(Self { document_id, doc })
    }

    pub fn from_snapshot(document_id: Uuid, snapshot: &[u8]) -> Result<Self, LoroDocumentError> {
        Self::from_snapshot_for_schema(document_id, snapshot, CURRENT_DOCUMENT_SCHEMA_VERSION)
    }

    pub fn from_snapshot_for_schema(
        document_id: Uuid,
        snapshot: &[u8],
        schema_version: i32,
    ) -> Result<Self, LoroDocumentError> {
        let doc = LoroDoc::from_snapshot(snapshot)
            .map_err(|error| LoroDocumentError::Import(error.to_string()))?;
        configure_loro_text_styles(&doc);
        let document = Self { document_id, doc };
        validate_tiptap_for_schema(&document.to_tiptap_unvalidated()?, schema_version)?;
        Ok(document)
    }

    pub fn document_id(&self) -> Uuid {
        self.document_id
    }

    pub fn peer_id(&self) -> u64 {
        self.doc.peer_id()
    }

    pub fn to_tiptap(&self) -> Result<Value, LoroDocumentError> {
        self.to_tiptap_for_schema(CURRENT_DOCUMENT_SCHEMA_VERSION)
    }

    pub fn to_tiptap_for_schema(&self, schema_version: i32) -> Result<Value, LoroDocumentError> {
        let value = self.to_tiptap_unvalidated()?;
        validate_tiptap_for_schema(&value, schema_version)?;
        Ok(value)
    }

    fn to_tiptap_unvalidated(&self) -> Result<Value, LoroDocumentError> {
        let root = self.doc.get_map(ROOT_CONTAINER);
        node_to_tiptap(&root)
    }

    pub fn sanitized_html(&self) -> Result<String, LoroDocumentError> {
        self.sanitized_html_for_schema(CURRENT_DOCUMENT_SCHEMA_VERSION)
    }

    pub fn sanitized_html_for_schema(
        &self,
        schema_version: i32,
    ) -> Result<String, LoroDocumentError> {
        tiptap_sanitized_html(&self.to_tiptap_for_schema(schema_version)?)
            .map_err(|error| LoroDocumentError::Invalid(error.to_string()))
    }

    pub fn plain_text(&self) -> Result<String, LoroDocumentError> {
        self.plain_text_for_schema(CURRENT_DOCUMENT_SCHEMA_VERSION)
    }

    pub fn plain_text_for_schema(&self, schema_version: i32) -> Result<String, LoroDocumentError> {
        Ok(tiptap_plain_text(
            &self.to_tiptap_for_schema(schema_version)?,
        ))
    }

    pub fn export_snapshot(&self) -> Result<Vec<u8>, LoroDocumentError> {
        self.doc
            .export(ExportMode::Snapshot)
            .map_err(loro_encode_error)
    }

    pub fn export_all_updates(&self) -> Result<Vec<u8>, LoroDocumentError> {
        self.doc
            .export(ExportMode::all_updates())
            .map_err(loro_encode_error)
    }

    pub fn export_updates_since(
        &self,
        version: &VersionVector,
    ) -> Result<Vec<u8>, LoroDocumentError> {
        self.doc
            .export(ExportMode::updates(version))
            .map_err(loro_encode_error)
    }

    pub fn version_vector(&self) -> VersionVector {
        self.doc.oplog_vv()
    }

    pub fn import_update(&mut self, update: &[u8]) -> Result<ImportStatus, LoroDocumentError> {
        self.import_update_for_schema(update, CURRENT_DOCUMENT_SCHEMA_VERSION)
    }

    pub fn import_update_for_schema(
        &mut self,
        update: &[u8],
        schema_version: i32,
    ) -> Result<ImportStatus, LoroDocumentError> {
        if update.is_empty() {
            return Err(LoroDocumentError::Invalid(
                "Loro update payload must not be empty".to_owned(),
            ));
        }

        let before = self.export_snapshot()?;
        let status = self
            .doc
            .import(update)
            .map_err(|error| LoroDocumentError::Import(error.to_string()))?;
        let validation = self
            .to_tiptap_unvalidated()
            .and_then(|value| validate_tiptap_for_schema(&value, schema_version));
        if let Err(error) = validation {
            let restored = LoroDoc::from_snapshot(&before)
                .map_err(|restore| LoroDocumentError::Import(restore.to_string()))?;
            configure_loro_text_styles(&restored);
            self.doc = restored;
            return Err(error);
        }
        Ok(status)
    }

    pub fn accept_update(
        &mut self,
        update_id: Uuid,
        principal_id: Uuid,
        source: impl Into<String>,
        payload: &[u8],
    ) -> Result<AcceptedLoroUpdate, LoroDocumentError> {
        self.accept_update_for_schema(
            update_id,
            principal_id,
            source,
            payload,
            CURRENT_DOCUMENT_SCHEMA_VERSION,
        )
    }

    pub fn accept_update_for_schema(
        &mut self,
        update_id: Uuid,
        principal_id: Uuid,
        source: impl Into<String>,
        payload: &[u8],
        schema_version: i32,
    ) -> Result<AcceptedLoroUpdate, LoroDocumentError> {
        let previous_frontiers = self.frontiers();
        self.import_update_for_schema(payload, schema_version)?;
        let resulting_frontiers = self.frontiers();

        Ok(AcceptedLoroUpdate {
            update_id,
            document_id: self.document_id,
            principal_id,
            source: source.into(),
            payload: payload.to_vec(),
            payload_sha256: sha256_hex(payload),
            previous_frontiers,
            resulting_frontiers,
            accepted_at: Utc::now(),
        })
    }

    pub fn frontiers(&self) -> Vec<LoroFrontier> {
        let mut frontiers = self
            .doc
            .state_frontiers()
            .iter()
            .map(|ID { peer, counter }| LoroFrontier {
                peer_id: peer,
                counter,
            })
            .collect::<Vec<_>>();
        frontiers.sort_unstable_by_key(|frontier| (frontier.peer_id, frontier.counter));
        frontiers
    }

    pub fn insert_text(
        &self,
        node_path: &[usize],
        offset: usize,
        text: &str,
    ) -> Result<(), LoroDocumentError> {
        let target = self.text_container(node_path)?;
        target
            .insert(offset, text)
            .map_err(|error| loro_import_error("editing text", error))
    }

    fn text_container(&self, node_path: &[usize]) -> Result<LoroText, LoroDocumentError> {
        let mut children = root_children(&self.doc)?;
        let mut target = None;
        for (depth, index) in node_path.iter().copied().enumerate() {
            let value = children.get(index).ok_or_else(|| {
                LoroDocumentError::Invalid("text node path is out of bounds".to_owned())
            })?;
            if depth + 1 == node_path.len() {
                target = Some(value);
            } else {
                let node = as_map(value)?;
                children = node
                    .get(CHILDREN_KEY)
                    .ok_or_else(|| {
                        LoroDocumentError::Invalid("text node path has no child content".to_owned())
                    })
                    .and_then(as_list)?;
            }
        }
        target
            .ok_or_else(|| {
                LoroDocumentError::Invalid("text node path must not be empty".to_owned())
            })
            .and_then(as_text)
    }
}

fn configured_loro_doc() -> LoroDoc {
    let doc = LoroDoc::new();
    configure_loro_text_styles(&doc);
    doc
}

fn configure_loro_text_styles(doc: &LoroDoc) {
    let mut styles = StyleConfigMap::default_rich_text_config();
    styles.insert(
        "strike".into(),
        StyleConfig {
            expand: ExpandType::After,
        },
    );
    doc.config_text_style(styles);
}

pub fn validate_tiptap_for_schema(
    value: &Value,
    schema_version: i32,
) -> Result<(), LoroDocumentError> {
    if value.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(LoroDocumentError::Invalid(
            "document root must have type doc".to_owned(),
        ));
    }
    if !matches!(schema_version, DOCUMENT_SCHEMA_V1 | DOCUMENT_SCHEMA_V2) {
        return Err(LoroDocumentError::Invalid(format!(
            "unsupported document schema version {schema_version}"
        )));
    }
    validate_schema_nodes(value, schema_version)?;
    tiptap_sanitized_html(value)
        .map(|_| ())
        .map_err(|error| LoroDocumentError::Invalid(error.to_string()))
}

pub fn is_supported_document_schema(schema_version: i32) -> bool {
    matches!(schema_version, DOCUMENT_SCHEMA_V1 | DOCUMENT_SCHEMA_V2)
}

pub fn migrate_v1_to_v2(value: &Value) -> Result<Value, LoroDocumentError> {
    validate_tiptap_for_schema(value, DOCUMENT_SCHEMA_V1)?;
    let migrated = migrate_node(value)?;
    validate_tiptap_for_schema(&migrated, DOCUMENT_SCHEMA_V2)?;
    Ok(migrated)
}

fn validate_schema_nodes(value: &Value, schema_version: i32) -> Result<(), LoroDocumentError> {
    let Some(object) = value.as_object() else {
        return Err(LoroDocumentError::Invalid(
            "document node must be an object".to_owned(),
        ));
    };
    if object.get("type").and_then(Value::as_str) == Some("callout") {
        if schema_version == DOCUMENT_SCHEMA_V1 {
            return Err(LoroDocumentError::Invalid(
                "callout nodes require document schema version 2".to_owned(),
            ));
        }
        let kind = object
            .get("attrs")
            .and_then(|attrs| attrs.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("info");
        if !matches!(kind, "info" | "warning" | "success" | "danger") {
            return Err(LoroDocumentError::Invalid(
                "callout kind is invalid".to_owned(),
            ));
        }
    }
    if let Some(children) = object.get("content").and_then(Value::as_array) {
        for child in children {
            validate_schema_nodes(child, schema_version)?;
        }
    }
    Ok(())
}

fn migrate_node(value: &Value) -> Result<Value, LoroDocumentError> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or_else(|| LoroDocumentError::Invalid("document node must be an object".to_owned()))?;
    if let Some(children) = object.get("content").and_then(Value::as_array) {
        let migrated = children
            .iter()
            .map(migrate_node)
            .collect::<Result<Vec<_>, _>>()?;
        object.insert("content".to_owned(), Value::Array(migrated));
    }
    if object.get("type").and_then(Value::as_str) == Some("blockquote") {
        object.insert("type".to_owned(), Value::String("callout".to_owned()));
        object.insert("attrs".to_owned(), serde_json::json!({"kind": "info"}));
    }
    Ok(Value::Object(object))
}

fn append_node(node: &LoroMap, value: &Value) -> Result<(), LoroDocumentError> {
    let object = value
        .as_object()
        .ok_or_else(|| LoroDocumentError::Invalid("document node must be an object".to_owned()))?;
    let node_type = object.get("type").and_then(Value::as_str).ok_or_else(|| {
        LoroDocumentError::Invalid("document node is missing its type".to_owned())
    })?;
    node.insert(NODE_NAME_KEY, node_type)
        .map_err(|error| loro_import_error("writing a document node type", error))?;

    let attributes = node
        .insert_container(ATTRIBUTES_KEY, LoroMap::new())
        .map_err(|error| loro_import_error("creating node attributes", error))?;
    if let Some(attrs) = object.get("attrs").and_then(Value::as_object) {
        for (key, value) in attrs {
            if !value.is_null() {
                attributes
                    .insert(key, value.clone())
                    .map_err(|error| loro_import_error("writing node attributes", error))?;
            }
        }
    }

    let children = node
        .insert_container(CHILDREN_KEY, LoroList::new())
        .map_err(|error| loro_import_error("creating node children", error))?;
    let nodes = object.get("content").and_then(Value::as_array);
    if node_type == "doc" && nodes.is_none_or(Vec::is_empty) {
        append_children(&children, &[serde_json::json!({"type": "paragraph"})])?;
    } else if let Some(nodes) = nodes {
        append_children(&children, nodes)?;
    }
    Ok(())
}

fn append_children(children: &LoroList, nodes: &[Value]) -> Result<(), LoroDocumentError> {
    let mut index = 0;
    while index < nodes.len() {
        if nodes[index].get("type").and_then(Value::as_str) == Some("text") {
            let text = children
                .push_container(LoroText::new())
                .map_err(|error| loro_import_error("creating a text node", error))?;
            let mut offset = 0;
            let mut marks = Vec::new();
            while index < nodes.len()
                && nodes[index].get("type").and_then(Value::as_str) == Some("text")
            {
                let object = nodes[index].as_object().ok_or_else(|| {
                    LoroDocumentError::Invalid("text node must be an object".to_owned())
                })?;
                let value = object.get("text").and_then(Value::as_str).ok_or_else(|| {
                    LoroDocumentError::Invalid("text node is missing text".to_owned())
                })?;
                text.insert(offset, value)
                    .map_err(|error| loro_import_error("writing a text node", error))?;
                let length = value.chars().count();
                if let Some(node_marks) = object.get("marks").and_then(Value::as_array) {
                    for mark in node_marks {
                        let mark_type =
                            mark.get("type").and_then(Value::as_str).ok_or_else(|| {
                                LoroDocumentError::Invalid(
                                    "text mark is missing its type".to_owned(),
                                )
                            })?;
                        let attrs = mark.get("attrs").cloned().unwrap_or_else(json_object);
                        marks.push((offset, offset + length, mark_type.to_owned(), attrs));
                    }
                }
                offset += length;
                index += 1;
            }
            for (from, to, mark_type, attrs) in marks {
                text.mark(from..to, &mark_type, attrs)
                    .map_err(|error| loro_import_error("writing text marks", error))?;
            }
        } else {
            let child = children
                .push_container(LoroMap::new())
                .map_err(|error| loro_import_error("creating a document child", error))?;
            append_node(&child, &nodes[index])?;
            index += 1;
        }
    }
    Ok(())
}

fn node_to_tiptap(node: &LoroMap) -> Result<Value, LoroDocumentError> {
    let node_type = node
        .get(NODE_NAME_KEY)
        .and_then(|value| match value {
            ValueOrContainer::Value(value) => value.as_string().map(ToOwned::to_owned),
            ValueOrContainer::Container(_) => None,
        })
        .ok_or_else(|| LoroDocumentError::Invalid("Loro node is missing nodeName".to_owned()))?;

    let mut object = Map::new();
    let is_doc = node_type.to_string() == "doc";
    object.insert("type".to_owned(), Value::String(node_type.to_string()));
    if let Some(attributes) = node.get(ATTRIBUTES_KEY).and_then(container_map) {
        let value = attributes.get_deep_value().to_json_value();
        if value.as_object().is_some_and(|attrs| !attrs.is_empty()) {
            object.insert("attrs".to_owned(), value);
        }
    }
    if let Some(children) = node.get(CHILDREN_KEY).and_then(container_list) {
        let content = children_to_tiptap(&children)?;
        if is_doc || content.as_array().is_some_and(|items| !items.is_empty()) {
            object.insert("content".to_owned(), content);
        }
    }
    Ok(Value::Object(object))
}

fn children_to_tiptap(children: &LoroList) -> Result<Value, LoroDocumentError> {
    let mut output = Vec::new();
    for index in 0..children.len() {
        let child = children.get(index).ok_or_else(|| {
            LoroDocumentError::Invalid("Loro child disappeared during projection".to_owned())
        })?;
        match child {
            ValueOrContainer::Container(Container::Map(map)) => {
                output.push(node_to_tiptap(&map)?);
            }
            ValueOrContainer::Container(Container::Text(text)) => {
                for delta in text.to_delta() {
                    let TextDelta::Insert { insert, attributes } = delta else {
                        continue;
                    };
                    let mut node = Map::new();
                    node.insert("type".to_owned(), Value::String("text".to_owned()));
                    node.insert("text".to_owned(), Value::String(insert));
                    if let Some(attributes) = attributes {
                        let mut marks = attributes.into_iter().collect::<Vec<_>>();
                        marks.sort_unstable_by(|left, right| left.0.cmp(&right.0));
                        node.insert(
                            "marks".to_owned(),
                            Value::Array(
                                marks
                                    .into_iter()
                                    .map(|(name, attrs)| {
                                        let mut mark = Map::new();
                                        mark.insert("type".to_owned(), Value::String(name));
                                        mark.insert("attrs".to_owned(), Value::from(attrs));
                                        Value::Object(mark)
                                    })
                                    .collect(),
                            ),
                        );
                    }
                    output.push(Value::Object(node));
                }
            }
            ValueOrContainer::Value(_) => {
                return Err(LoroDocumentError::Invalid(
                    "Loro document children must be containers".to_owned(),
                ));
            }
            ValueOrContainer::Container(_) => {
                return Err(LoroDocumentError::Invalid(
                    "unsupported Loro document child container".to_owned(),
                ));
            }
        }
    }
    Ok(Value::Array(output))
}

fn root_children(doc: &LoroDoc) -> Result<LoroList, LoroDocumentError> {
    doc.get_map(ROOT_CONTAINER)
        .get(CHILDREN_KEY)
        .ok_or_else(|| LoroDocumentError::Invalid("document has no children".to_owned()))
        .and_then(as_list)
}

fn container_map(value: ValueOrContainer) -> Option<LoroMap> {
    match value {
        ValueOrContainer::Container(Container::Map(map)) => Some(map),
        _ => None,
    }
}

fn container_list(value: ValueOrContainer) -> Option<LoroList> {
    match value {
        ValueOrContainer::Container(Container::List(list)) => Some(list),
        _ => None,
    }
}

fn as_list(value: ValueOrContainer) -> Result<LoroList, LoroDocumentError> {
    container_list(value)
        .ok_or_else(|| LoroDocumentError::Invalid("document children are not a list".to_owned()))
}

fn as_map(value: ValueOrContainer) -> Result<LoroMap, LoroDocumentError> {
    container_map(value)
        .ok_or_else(|| LoroDocumentError::Invalid("document node is not a map".to_owned()))
}

fn as_text(value: ValueOrContainer) -> Result<LoroText, LoroDocumentError> {
    match value {
        ValueOrContainer::Container(Container::Text(text)) => Ok(text),
        _ => Err(LoroDocumentError::Invalid(
            "target node is not a text container".to_owned(),
        )),
    }
}

fn json_object() -> Value {
    Value::Object(Map::new())
}

fn loro_encode_error(error: LoroEncodeError) -> LoroDocumentError {
    LoroDocumentError::Encode(error.to_string())
}

fn loro_import_error(context: &str, error: impl std::fmt::Display) -> LoroDocumentError {
    LoroDocumentError::Import(format!("{context}: {error}"))
}

fn sha256_hex(payload: &[u8]) -> String {
    Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn document() -> Value {
        json!({
            "type": "doc",
            "content": [
                {
                    "type": "heading",
                    "attrs": {"level": 2},
                    "content": [{"type": "text", "text": "A title", "marks": [{"type": "bold", "attrs": {}}]}]
                },
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "hello"},
                        {"type": "riichiLink", "attrs": {"resourceType": "issue", "resourceId": "RII-1", "label": "RII-1"}}
                    ]
                },
                {"type": "image", "attrs": {"attachmentId": "att-1", "src": "/api/attachments/att-1", "alt": "diagram"}}
            ]
        })
    }

    fn current_document_editor_schema() -> Value {
        json!({
            "type": "doc",
            "content": [
                {
                    "type": "heading",
                    "attrs": {"level": 2},
                    "content": [{"type": "text", "text": "Specification", "marks": [{"type": "bold", "attrs": {}}]}]
                },
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "plain "},
                        {"type": "text", "text": "styled", "marks": [
                            {"type": "italic", "attrs": {}},
                            {"type": "strike", "attrs": {}},
                            {"type": "code", "attrs": {}}
                        ]},
                        {"type": "text", "text": " "},
                        {"type": "text", "text": "docs", "marks": [{"type": "link", "attrs": {"href": "https://example.com"}}]},
                        {"type": "hardBreak"},
                        {"type": "mention", "attrs": {"id": "account-1", "label": "Alex"}}
                    ]
                },
                {"type": "bulletList", "content": [
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "bullet"}]}]}
                ]},
                {"type": "orderedList", "attrs": {"start": 3}, "content": [
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "ordered"}]}]}
                ]},
                {"type": "taskList", "content": [
                    {"type": "taskItem", "attrs": {"checked": true}, "content": [{"type": "paragraph", "content": [{"type": "text", "text": "finished"}]}]}
                ]},
                {"type": "blockquote", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "quote"}]}]},
                {"type": "codeBlock", "attrs": {"language": "rust"}, "content": [{"type": "text", "text": "fn main() {}"}]},
                {"type": "horizontalRule"},
                {"type": "image", "attrs": {"attachmentId": "att-1", "src": "/api/attachments/att-1", "alt": "diagram"}}
            ]
        })
    }

    #[test]
    fn v2_callouts_are_rejected_by_v1_and_migrate_from_blockquotes() {
        let v1 = json!({
            "type": "doc",
            "content": [{
                "type": "blockquote",
                "content": [{
                    "type": "paragraph",
                    "content": [{"type": "text", "text": "migration note"}]
                }]
            }]
        });
        assert!(validate_tiptap_for_schema(&v1, DOCUMENT_SCHEMA_V1).is_ok());

        let migrated = migrate_v1_to_v2(&v1).unwrap();
        assert_eq!(migrated["content"][0]["type"], "callout");
        assert_eq!(migrated["content"][0]["attrs"]["kind"], "info");
        assert!(validate_tiptap_for_schema(&migrated, DOCUMENT_SCHEMA_V2).is_ok());

        let callout = json!({
            "type": "doc",
            "content": [{
                "type": "callout",
                "attrs": {"kind": "warning"},
                "content": [{"type": "paragraph", "content": [{"type": "text", "text": "warning"}]}]
            }]
        });
        assert!(validate_tiptap_for_schema(&callout, DOCUMENT_SCHEMA_V1).is_err());
        assert!(validate_tiptap_for_schema(&callout, DOCUMENT_SCHEMA_V2).is_ok());
    }

    fn normalize_mark_order(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(Value::Array(marks)) = object.get_mut("marks") {
                    marks.sort_by_key(|mark| {
                        mark.get("type")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned()
                    });
                }
                for child in object.values_mut() {
                    normalize_mark_order(child);
                }
            }
            Value::Array(values) => {
                for value in values {
                    normalize_mark_order(value);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn normalizes_empty_documents_to_a_paragraph_for_editor_schemas() {
        let loro = LoroDocument::from_tiptap(
            Uuid::nil(),
            &serde_json::json!({"type": "doc", "content": []}),
        )
        .unwrap();

        assert_eq!(
            loro.to_tiptap().unwrap(),
            serde_json::json!({
                "type": "doc",
                "content": [{"type": "paragraph"}]
            })
        );
    }

    #[test]
    fn preserves_tiptap_shape_and_existing_projections() {
        let expected = document();
        let loro = LoroDocument::from_tiptap(Uuid::nil(), &expected).unwrap();

        assert_eq!(loro.to_tiptap().unwrap(), expected);
        assert_eq!(loro.plain_text().unwrap(), "A title\nhello\nRII-1");
        assert_eq!(
            loro.sanitized_html().unwrap(),
            "<h2><strong>A title</strong></h2><p>hello<a data-riichi-link data-resource-kind=\"issue\" data-resource-id=\"RII-1\" href=\"#/issue/RII-1\">RII-1</a></p><span data-attachment=\"att-1\">[attachment]</span>"
        );
    }

    #[test]
    fn round_trips_the_current_document_editor_schema() {
        let mut expected = current_document_editor_schema();
        let loro = LoroDocument::from_tiptap(Uuid::nil(), &expected).unwrap();
        let mut actual = loro.to_tiptap().unwrap();
        normalize_mark_order(&mut expected);
        normalize_mark_order(&mut actual);

        assert_eq!(actual, expected);
        assert!(loro.plain_text().unwrap().contains("Specification"));
        assert!(loro.plain_text().unwrap().contains("finished"));
        let html = loro.sanitized_html().unwrap();
        assert!(html.contains("data-mention"));
        assert!(html.contains("data-type=\"taskList\""));
        assert!(html.contains("data-checked=\"true\""));
    }

    #[test]
    fn snapshots_restore_the_same_document() {
        let expected = document();
        let original = LoroDocument::from_tiptap(Uuid::nil(), &expected).unwrap();
        let snapshot = original.export_snapshot().unwrap();
        let restored = LoroDocument::from_snapshot(Uuid::nil(), &snapshot).unwrap();

        assert_eq!(restored.to_tiptap().unwrap(), expected);
        assert_eq!(restored.frontiers(), original.frontiers());
    }

    #[test]
    fn shallow_snapshot_recovery_preserves_state_and_accepts_new_updates() {
        let original = LoroDocument::from_tiptap(Uuid::nil(), &document()).unwrap();
        original.doc.commit();
        let shallow_frontiers = original.doc.oplog_frontiers();
        let snapshot = original
            .doc
            .export(ExportMode::shallow_snapshot(&shallow_frontiers))
            .unwrap();
        let mut recovered = LoroDocument::from_snapshot(Uuid::nil(), &snapshot).unwrap();

        assert_eq!(
            recovered.to_tiptap().unwrap(),
            original.to_tiptap().unwrap()
        );
        assert_eq!(recovered.frontiers(), original.frontiers());
        assert_eq!(recovered.doc.shallow_since_frontiers(), shallow_frontiers);

        let recovered_version = recovered.version_vector();
        original.insert_text(&[1, 0], 5, " after recovery").unwrap();
        let update = original.export_updates_since(&recovered_version).unwrap();
        recovered.import_update(&update).unwrap();

        assert_eq!(
            recovered.to_tiptap().unwrap(),
            original.to_tiptap().unwrap()
        );
    }

    #[test]
    fn updates_can_be_applied_to_a_snapshot_replica() {
        let expected = document();
        let writer = LoroDocument::from_tiptap(Uuid::nil(), &expected).unwrap();
        let snapshot = writer.export_snapshot().unwrap();
        let mut replica = LoroDocument::from_snapshot(Uuid::nil(), &snapshot).unwrap();

        writer.insert_text(&[1, 0], 5, " there").unwrap();
        let update = writer
            .export_updates_since(&replica.doc.oplog_vv())
            .unwrap();
        replica.import_update(&update).unwrap();

        assert_eq!(replica.to_tiptap().unwrap(), writer.to_tiptap().unwrap());
        assert_eq!(replica.plain_text().unwrap(), "A title\nhello there\nRII-1");
    }

    #[test]
    fn concurrent_replicas_converge_after_exchanging_encoded_updates() {
        let original = LoroDocument::from_tiptap(Uuid::nil(), &document()).unwrap();
        let snapshot = original.export_snapshot().unwrap();
        let mut left = LoroDocument::from_snapshot(Uuid::nil(), &snapshot).unwrap();
        let mut right = LoroDocument::from_snapshot(Uuid::nil(), &snapshot).unwrap();
        assert_ne!(left.peer_id(), right.peer_id());

        let base_left = left.version_vector();
        let base_right = right.version_vector();
        left.insert_text(&[1, 0], 5, " left").unwrap();
        right.insert_text(&[1, 0], 5, " right").unwrap();
        let left_update = left.export_updates_since(&base_left).unwrap();
        let right_update = right.export_updates_since(&base_right).unwrap();

        left.import_update(&right_update).unwrap();
        right.import_update(&left_update).unwrap();

        assert_eq!(left.to_tiptap().unwrap(), right.to_tiptap().unwrap());
        assert_eq!(left.frontiers(), right.frontiers());
        assert!(left.plain_text().unwrap().contains("left"));
        assert!(left.plain_text().unwrap().contains("right"));
    }

    #[test]
    fn accepted_updates_include_a_verifiable_envelope() {
        let expected = document();
        let writer = LoroDocument::from_tiptap(Uuid::nil(), &expected).unwrap();
        let snapshot = writer.export_snapshot().unwrap();
        let mut replica = LoroDocument::from_snapshot(Uuid::nil(), &snapshot).unwrap();
        writer.insert_text(&[1, 0], 5, " there").unwrap();
        let update = writer
            .export_updates_since(&replica.doc.oplog_vv())
            .unwrap();

        let envelope = replica
            .accept_update(Uuid::from_u128(1), Uuid::from_u128(2), "human", &update)
            .unwrap();

        assert_eq!(envelope.document_id, Uuid::nil());
        assert_eq!(envelope.payload, update);
        assert_eq!(envelope.payload_sha256, sha256_hex(&update));
        assert_ne!(envelope.previous_frontiers, envelope.resulting_frontiers);
    }

    #[test]
    fn invalid_tiptap_is_rejected_before_hydration() {
        let invalid = json!({
            "type": "doc",
            "content": [{"type": "unknown"}]
        });

        assert!(matches!(
            LoroDocument::from_tiptap(Uuid::nil(), &invalid),
            Err(LoroDocumentError::Invalid(_))
        ));
    }
}
