use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use axum::{
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};
use riichi_application::loro_document::{
    CURRENT_DOCUMENT_SCHEMA_VERSION, LoroSnapshot, LoroUpdateCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::{ApiError, AppState, human_principal};

const CHANNEL_CAPACITY: usize = 256;
const MAX_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
const AUTHORIZATION_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

#[derive(Clone)]
pub(super) struct DocumentSyncRegistry {
    channels: Arc<Mutex<HashMap<Uuid, broadcast::Sender<BroadcastUpdate>>>>,
    seen_updates: Arc<Mutex<HashSet<Uuid>>>,
    active_peers: Arc<Mutex<HashMap<(Uuid, String), Uuid>>>,
}

impl DocumentSyncRegistry {
    pub(super) fn new() -> Self {
        Self {
            channels: Arc::new(Mutex::new(HashMap::new())),
            seen_updates: Arc::new(Mutex::new(HashSet::new())),
            active_peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn subscribe(&self, document_id: Uuid) -> broadcast::Receiver<BroadcastUpdate> {
        let mut channels = self
            .channels
            .lock()
            .expect("document sync registry poisoned");
        channels
            .entry(document_id)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .subscribe()
    }

    pub(super) fn publish(&self, document_id: Uuid, update: BroadcastUpdate) {
        let mut seen_updates = self
            .seen_updates
            .lock()
            .expect("document sync registry poisoned");
        if !seen_updates.insert(update.update_id) {
            return;
        }
        if seen_updates.len() > 4096 {
            seen_updates.clear();
            seen_updates.insert(update.update_id);
        }
        drop(seen_updates);
        let channels = self
            .channels
            .lock()
            .expect("document sync registry poisoned");
        if let Some(sender) = channels.get(&document_id) {
            let _ = sender.send(update);
        }
    }

    fn remove_if_unused(&self, document_id: Uuid) {
        let mut channels = self
            .channels
            .lock()
            .expect("document sync registry poisoned");
        if channels
            .get(&document_id)
            .is_some_and(|sender| sender.receiver_count() == 0)
        {
            channels.remove(&document_id);
        }
    }

    fn claim_peer(&self, document_id: Uuid, peer_id: &str, connection_id: Uuid) -> bool {
        let mut active_peers = self
            .active_peers
            .lock()
            .expect("document sync peer registry poisoned");
        let key = (document_id, peer_id.to_owned());
        match active_peers.get(&key) {
            Some(existing) if *existing != connection_id => false,
            _ => {
                active_peers.insert(key, connection_id);
                true
            }
        }
    }

    fn release_connection(&self, document_id: Uuid, connection_id: Uuid) {
        let mut active_peers = self
            .active_peers
            .lock()
            .expect("document sync peer registry poisoned");
        active_peers.retain(|(active_document_id, _), active_connection_id| {
            *active_document_id != document_id || *active_connection_id != connection_id
        });
    }
}

struct DocumentSyncConnectionGuard {
    registry: DocumentSyncRegistry,
    document_id: Uuid,
    connection_id: Uuid,
}

impl Drop for DocumentSyncConnectionGuard {
    fn drop(&mut self) {
        self.registry
            .release_connection(self.document_id, self.connection_id);
        self.registry.remove_if_unused(self.document_id);
    }
}

pub(super) fn spawn_document_sync_hub(
    database: riichi_persistence::Database,
    registry: DocumentSyncRegistry,
) {
    tokio::spawn(async move {
        loop {
            let mut listener = match database.loro_update_listener().await {
                Ok(listener) => listener,
                Err(error) => {
                    tracing::warn!(%error, "Loro sync listener unavailable");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };
            loop {
                let notification = match listener.recv().await {
                    Ok(notification) => notification,
                    Err(error) => {
                        tracing::warn!(%error, "Loro sync listener disconnected");
                        break;
                    }
                };
                let payload: Value = match serde_json::from_str(notification.payload()) {
                    Ok(payload) => payload,
                    Err(_) => continue,
                };
                let Some(document_id) = payload
                    .get("document_id")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse().ok())
                else {
                    continue;
                };
                let Some(update_id) = payload
                    .get("update_id")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse().ok())
                else {
                    continue;
                };
                let Ok(Some(update)) = database
                    .get_loro_update_for_broadcast(document_id, update_id)
                    .await
                else {
                    continue;
                };
                let resulting_frontiers = match serde_json::from_value(update.resulting_frontiers) {
                    Ok(frontiers) => frontiers,
                    Err(_) => continue,
                };
                registry.publish(
                    document_id,
                    BroadcastUpdate {
                        origin_connection_id: Uuid::nil(),
                        update_id: update.update_id,
                        payload: update.payload,
                        resulting_frontiers,
                    },
                );
            }
        }
    });
}

#[derive(Debug, Clone)]
pub(super) struct BroadcastUpdate {
    origin_connection_id: Uuid,
    update_id: Uuid,
    payload: Vec<u8>,
    resulting_frontiers: Vec<Frontier>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Hello {
        #[serde(default)]
        peer_id: Option<String>,
        #[serde(default)]
        schema_version: Option<i32>,
    },
    Update {
        update_id: Uuid,
        idempotency_key: Option<String>,
        payload_base64: String,
    },
}

#[derive(Debug, Deserialize)]
struct BinaryUpdateEnvelope {
    r#type: String,
    update_id: Uuid,
    idempotency_key: Option<String>,
}

struct ClientUpdate {
    update_id: Uuid,
    idempotency_key: Option<String>,
    payload: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage {
    Accepted {
        update_id: Uuid,
        document_id: Uuid,
        resulting_frontiers: Vec<Frontier>,
        accepted_at: DateTime<Utc>,
        replayed: bool,
    },
    Error {
        update_id: Option<Uuid>,
        retryable: bool,
        message: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BinaryServerMessage {
    Snapshot {
        document_id: Uuid,
        revision: i64,
        schema_version: i32,
        frontiers: Vec<Frontier>,
    },
    Update {
        update_id: Uuid,
        resulting_frontiers: Vec<Frontier>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Frontier {
    peer_id: String,
    counter: i32,
}

pub(super) async fn document_loro_sync(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
    websocket: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let receiver = state.document_sync.subscribe(document_id);
    let snapshot = match state
        .application
        .get_loro_snapshot(principal.account.id, document_id, None)
        .await
    {
        Ok(snapshot) => snapshot,
        Err(error) => {
            drop(receiver);
            state.document_sync.remove_if_unused(document_id);
            return Err(ApiError::from(error));
        }
    };
    let connection_id = Uuid::now_v7();
    let account_id = principal.account.id;
    let access_wakeups = state.event_wakeups.subscribe();
    Ok(websocket.on_upgrade(move |socket| {
        run_document_socket(
            socket,
            DocumentSocketContext {
                state,
                account_id,
                document_id,
                connection_id,
                snapshot,
                receiver,
                access_wakeups,
            },
        )
    }))
}

struct DocumentSocketContext {
    state: AppState,
    account_id: Uuid,
    document_id: Uuid,
    connection_id: Uuid,
    snapshot: riichi_application::loro_document::LoroSnapshot,
    receiver: broadcast::Receiver<BroadcastUpdate>,
    access_wakeups: broadcast::Receiver<super::EventWakeup>,
}

async fn run_document_socket(mut socket: WebSocket, context: DocumentSocketContext) {
    let DocumentSocketContext {
        state,
        account_id,
        document_id,
        connection_id,
        snapshot,
        mut receiver,
        mut access_wakeups,
    } = context;
    let _connection_guard = DocumentSyncConnectionGuard {
        registry: state.document_sync.clone(),
        document_id,
        connection_id,
    };
    let mut pending_snapshot = Some(snapshot);
    let mut handshake_complete = false;
    let mut client_schema_version = None;

    let mut authorization_check = tokio::time::interval(AUTHORIZATION_CHECK_INTERVAL);
    authorization_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            access_change = access_wakeups.recv() => {
                match access_change {
                    Ok(super::EventWakeup::Account(changed_account_id)) if changed_account_id == account_id => {
                        let _ = send_json(&mut socket, ServerMessage::Error {
                            update_id: None,
                            retryable: false,
                            message: "document access was revoked".to_owned(),
                        }).await;
                        return;
                    }
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => {}
                }
            }
            _ = authorization_check.tick() => {
                match state.application.document_is_accessible(account_id, document_id).await {
                    Ok(true) => {}
                    Ok(false) => {
                        let _ = send_json(&mut socket, ServerMessage::Error {
                            update_id: None,
                            retryable: false,
                            message: "document access was revoked".to_owned(),
                        }).await;
                        return;
                    }
                    Err(_) => {
                        let _ = send_json(&mut socket, ServerMessage::Error {
                            update_id: None,
                            retryable: true,
                            message: "document access could not be verified".to_owned(),
                        }).await;
                        return;
                    }
                }
            }
            incoming = socket.recv() => {
                let Some(Ok(message)) = incoming else { return };
                match message {
                    Message::Text(text) => {
                        if text.len() > MAX_MESSAGE_BYTES {
                            let _ = send_json(&mut socket, ServerMessage::Error {
                                update_id: None,
                                retryable: false,
                                message: "sync message is too large".to_owned(),
                            }).await;
                            return;
                        }
                        if handle_client_message(ClientMessageContext {
                            socket: &mut socket,
                            state: &state,
                            account_id,
                            document_id,
                            connection_id,
                            pending_snapshot: &mut pending_snapshot,
                            handshake_complete: &mut handshake_complete,
                            client_schema_version: &mut client_schema_version,
                        }, text.as_str()).await.is_err() {
                            return;
                        }
                    }
                    Message::Ping(payload) => {
                        if socket.send(Message::Pong(payload)).await.is_err() { return; }
                    }
                    Message::Close(_) => return,
                    Message::Binary(bytes) => {
                        if bytes.len() > MAX_MESSAGE_BYTES {
                            let _ = send_json(
                                &mut socket,
                                ServerMessage::Error {
                                    update_id: None,
                                    retryable: false,
                                    message: "sync message is too large".to_owned(),
                                },
                            )
                            .await;
                            return;
                        }
                        if handle_binary_update(
                            BinaryUpdateContext {
                                socket: &mut socket,
                                state: &state,
                                account_id,
                                document_id,
                                connection_id,
                                handshake_complete,
                                client_schema_version,
                            },
                            bytes.to_vec(),
                        )
                        .await
                        .is_err()
                        {
                            return;
                        }
                    }
                    Message::Pong(_) => {
                        let _ = send_json(&mut socket, ServerMessage::Error {
                            update_id: None,
                            retryable: false,
                            message: "sync clients must send JSON text messages".to_owned(),
                        }).await;
                    }
                }
            }
            update = receiver.recv() => {
                match update {
                    Ok(update) if handshake_complete && update.origin_connection_id != connection_id => {
                        if send_binary(&mut socket, BinaryServerMessage::Update {
                            update_id: update.update_id,
                            resulting_frontiers: update.resulting_frontiers,
                        }, update.payload).await.is_err() { return; }
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let Ok(snapshot) = state.application.get_loro_snapshot(account_id, document_id, None).await else { return; };
                        if send_binary(&mut socket, BinaryServerMessage::Snapshot {
                            document_id,
                            revision: snapshot.revision,
                            schema_version: snapshot.schema_version,
                            frontiers: snapshot.frontiers.into_iter().map(frontier).collect(),
                        }, snapshot.bytes).await.is_err() { return; }
                    }
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
        }
    }
}

struct ClientMessageContext<'a> {
    socket: &'a mut WebSocket,
    state: &'a AppState,
    account_id: Uuid,
    document_id: Uuid,
    connection_id: Uuid,
    pending_snapshot: &'a mut Option<LoroSnapshot>,
    handshake_complete: &'a mut bool,
    client_schema_version: &'a mut Option<i32>,
}

async fn handle_client_message(context: ClientMessageContext<'_>, text: &str) -> Result<(), ()> {
    let ClientMessageContext {
        socket,
        state,
        account_id,
        document_id,
        connection_id,
        pending_snapshot,
        handshake_complete,
        client_schema_version,
    } = context;
    let message: ClientMessage = match serde_json::from_str(text) {
        Ok(message) => message,
        Err(_) => {
            send_json(
                socket,
                ServerMessage::Error {
                    update_id: None,
                    retryable: false,
                    message: "invalid sync message".to_owned(),
                },
            )
            .await
            .map_err(|_| ())?;
            return Ok(());
        }
    };
    match message {
        ClientMessage::Hello {
            peer_id,
            schema_version,
        } => {
            let Some(snapshot) = pending_snapshot.as_ref() else {
                send_json(
                    socket,
                    ServerMessage::Error {
                        update_id: None,
                        retryable: false,
                        message: "document handshake is already complete".to_owned(),
                    },
                )
                .await
                .map_err(|_| ())?;
                return Err(());
            };
            let negotiated_schema_version =
                schema_version.unwrap_or(CURRENT_DOCUMENT_SCHEMA_VERSION);
            if negotiated_schema_version != snapshot.schema_version {
                send_json(
                    socket,
                    ServerMessage::Error {
                        update_id: None,
                        retryable: false,
                        message: format!(
                            "document schema version {negotiated_schema_version} is incompatible with server version {}",
                            snapshot.schema_version
                        ),
                    },
                )
                .await
                .map_err(|_| ())?;
                return Err(());
            }
            let Some(peer_id) = peer_id else {
                send_json(
                    socket,
                    ServerMessage::Error {
                        update_id: None,
                        retryable: false,
                        message: "document peer ID is required".to_owned(),
                    },
                )
                .await
                .map_err(|_| ())?;
                return Err(());
            };
            if peer_id.trim().is_empty()
                || peer_id.len() > 64
                || peer_id.parse::<u64>().is_err()
                || !state
                    .document_sync
                    .claim_peer(document_id, &peer_id, connection_id)
            {
                send_json(
                    socket,
                    ServerMessage::Error {
                        update_id: None,
                        retryable: false,
                        message: "document peer ID is already active or invalid".to_owned(),
                    },
                )
                .await
                .map_err(|_| ())?;
                return Err(());
            }
            let snapshot = pending_snapshot.take().expect("snapshot checked above");
            send_binary(
                socket,
                BinaryServerMessage::Snapshot {
                    document_id,
                    revision: snapshot.revision,
                    schema_version: snapshot.schema_version,
                    frontiers: snapshot.frontiers.into_iter().map(frontier).collect(),
                },
                snapshot.bytes,
            )
            .await
            .map_err(|_| ())?;
            *handshake_complete = true;
            *client_schema_version = Some(negotiated_schema_version);
            Ok(())
        }
        ClientMessage::Update {
            update_id,
            idempotency_key,
            payload_base64,
        } => {
            if !*handshake_complete {
                send_json(
                    socket,
                    ServerMessage::Error {
                        update_id: None,
                        retryable: false,
                        message: "document handshake is required before updates".to_owned(),
                    },
                )
                .await
                .map_err(|_| ())?;
                return Err(());
            }
            let Some(client_schema_version) = *client_schema_version else {
                return Err(());
            };
            let payload = match BASE64.decode(payload_base64) {
                Ok(payload) => payload,
                Err(_) => {
                    send_json(
                        socket,
                        ServerMessage::Error {
                            update_id: None,
                            retryable: false,
                            message: "sync update payload is invalid".to_owned(),
                        },
                    )
                    .await
                    .map_err(|_| ())?;
                    return Ok(());
                }
            };
            accept_client_update(
                socket,
                state,
                account_id,
                document_id,
                connection_id,
                client_schema_version,
                ClientUpdate {
                    update_id,
                    idempotency_key,
                    payload,
                },
            )
            .await
        }
    }
}

struct BinaryUpdateContext<'a> {
    socket: &'a mut WebSocket,
    state: &'a AppState,
    account_id: Uuid,
    document_id: Uuid,
    connection_id: Uuid,
    handshake_complete: bool,
    client_schema_version: Option<i32>,
}

async fn handle_binary_update(context: BinaryUpdateContext<'_>, bytes: Vec<u8>) -> Result<(), ()> {
    let BinaryUpdateContext {
        socket,
        state,
        account_id,
        document_id,
        connection_id,
        handshake_complete,
        client_schema_version,
    } = context;
    if !handshake_complete {
        send_json(
            socket,
            ServerMessage::Error {
                update_id: None,
                retryable: false,
                message: "document handshake is required before updates".to_owned(),
            },
        )
        .await
        .map_err(|_| ())?;
        return Err(());
    }
    let Some(client_schema_version) = client_schema_version else {
        return Err(());
    };
    let Some(separator) = bytes.iter().position(|byte| *byte == b'\n') else {
        send_json(
            socket,
            ServerMessage::Error {
                update_id: None,
                retryable: false,
                message: "binary sync message has no envelope".to_owned(),
            },
        )
        .await
        .map_err(|_| ())?;
        return Ok(());
    };
    let envelope: BinaryUpdateEnvelope =
        match serde_json::from_slice::<BinaryUpdateEnvelope>(&bytes[..separator]) {
            Ok(envelope) if envelope.r#type == "update" => envelope,
            _ => {
                send_json(
                    socket,
                    ServerMessage::Error {
                        update_id: None,
                        retryable: false,
                        message: "binary sync envelope is invalid".to_owned(),
                    },
                )
                .await
                .map_err(|_| ())?;
                return Ok(());
            }
        };
    accept_client_update(
        socket,
        state,
        account_id,
        document_id,
        connection_id,
        client_schema_version,
        ClientUpdate {
            update_id: envelope.update_id,
            idempotency_key: envelope.idempotency_key,
            payload: bytes[separator + 1..].to_vec(),
        },
    )
    .await
}

async fn accept_client_update(
    socket: &mut WebSocket,
    state: &AppState,
    account_id: Uuid,
    document_id: Uuid,
    connection_id: Uuid,
    schema_version: i32,
    update: ClientUpdate,
) -> Result<(), ()> {
    let result = match state
        .application
        .accept_loro_transport_update(
            account_id,
            document_id,
            LoroUpdateCommand {
                schema_version,
                update_id: update.update_id,
                idempotency_key: update.idempotency_key,
                previous_frontiers: Vec::new(),
                payload: update.payload.clone(),
                source: "human:websocket".to_owned(),
            },
        )
        .await
    {
        Ok(result) => result,
        Err(error) => {
            async_error(socket, error, Some(update.update_id)).await;
            return Ok(());
        }
    };
    send_json(
        socket,
        ServerMessage::Accepted {
            update_id: result.update_id,
            document_id: result.document_id,
            resulting_frontiers: result
                .resulting_frontiers
                .iter()
                .cloned()
                .map(frontier)
                .collect(),
            accepted_at: result.accepted_at,
            replayed: result.replayed,
        },
    )
    .await
    .map_err(|_| ())?;
    if !result.replayed {
        state.document_sync.publish(
            document_id,
            BroadcastUpdate {
                origin_connection_id: connection_id,
                update_id: result.update_id,
                payload: update.payload,
                resulting_frontiers: result
                    .resulting_frontiers
                    .into_iter()
                    .map(frontier)
                    .collect(),
            },
        );
    }
    Ok(())
}

async fn async_error(
    socket: &mut WebSocket,
    error: riichi_persistence::Error,
    update_id: Option<Uuid>,
) {
    let retryable = matches!(
        error,
        riichi_persistence::Error::LoroFrontierConflict | riichi_persistence::Error::Database(_)
    );
    let message = match error {
        riichi_persistence::Error::DocumentNotFound => "document was not found",
        riichi_persistence::Error::DocumentAccessDenied => "you don't have access to this document",
        riichi_persistence::Error::LoroFrontierConflict => "document changed; reconnect and retry",
        riichi_persistence::Error::InvalidDocument(_) => "document update is invalid",
        riichi_persistence::Error::Database(_) => "document sync is temporarily unavailable",
        _ => "document update could not be accepted",
    };
    let _ = send_json(
        socket,
        ServerMessage::Error {
            update_id,
            retryable,
            message: message.to_owned(),
        },
    )
    .await;
}

async fn send_json(socket: &mut WebSocket, message: ServerMessage) -> Result<(), String> {
    let text = serde_json::to_string(&message).map_err(|error| error.to_string())?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|error| error.to_string())
}

async fn send_binary(
    socket: &mut WebSocket,
    message: BinaryServerMessage,
    payload: Vec<u8>,
) -> Result<(), String> {
    let envelope = serde_json::to_vec(&message).map_err(|error| error.to_string())?;
    let mut frame = Vec::with_capacity(envelope.len() + 1 + payload.len());
    frame.extend_from_slice(&envelope);
    frame.push(b'\n');
    frame.extend_from_slice(&payload);
    socket
        .send(Message::Binary(frame.into()))
        .await
        .map_err(|error| error.to_string())
}

fn frontier(frontier: riichi_application::loro_document::LoroFrontier) -> Frontier {
    Frontier {
        peer_id: frontier.peer_id.to_string(),
        counter: frontier.counter,
    }
}
