//! ui-acceptor.
//!
//! Owns the TCP listen socket for the inbox UI. On each accepted
//! connection, spawns a fresh ui-handler child via supervisor.spawn,
//! transfers the connection to it, and forgets about it. The handler
//! exits after serving its single HTTP request.
//!
//! Mirrors inbox/acceptor exactly except that we have no DKIM, no
//! mailbox-router, and no SMTP — a UI request is fully self-contained
//! and stateless across requests.
//!
//! initial_state (JSON, one line):
//!   {
//!     "bearer_token":       "<API bearer used by handler for /api/send>",
//!     "basic_auth":         "<htpasswd-style user:password for UI access>",
//!     "api_base_url":       "https://mail.colinrozzi.com:443",
//!     "ui_handler_manifest":"<theater resolve_reference>"
//!   }
//!
//! `bearer_token` and `basic_auth` are persisted to the shared store at
//! init so the per-connection handler can pull them without us passing
//! secrets through init_state on every spawn.

#![no_std]
extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use packr_guest::{export, import, pack_types, GraphValue, Value};
use serde::Deserialize;

packr_guest::setup_guest!();

#[derive(Clone, GraphValue)]
#[graph(crate = "packr_guest::composite_abi")]
pub struct AcceptorState {
    pub listener_id: String,
    pub ui_handler_manifest: String,
    pub api_base_url: String,
}

pack_types! {
    imports {
        theater:simple/runtime {
            log: func(msg: string),
        }
        theater:simple/tcp {
            listen: func(address: string) -> result<string, string>,
            transfer: func(connection-id: string, target-actor: string) -> result<_, string>,
        }
        theater:simple/supervisor {
            spawn: func(manifest: string, init-state: option<value>, wasm-bytes: option<list<u8>>) -> result<string, string>,
            stop-child: func(child-id: string) -> result<_, string>,
        }
        theater:simple/store {
            store-at-label: func(store-id: string, label: string, content: list<u8>) -> result<string, string>,
        }
    }
    exports {
        theater:simple/actor.init: func(state: value) -> result<acceptor-state, string>,
        theater:simple/tcp-client.handle-connection: func(state: acceptor-state, connection-id: string) -> result<acceptor-state, string>,
    }
}

#[import(module = "theater:simple/runtime", name = "log")]
fn log(msg: String);

#[import(module = "theater:simple/tcp", name = "listen")]
fn tcp_listen(address: String) -> Result<String, String>;

#[import(module = "theater:simple/tcp", name = "transfer")]
fn tcp_transfer(connection_id: String, target_actor: String) -> Result<(), String>;

#[import(module = "theater:simple/supervisor", name = "spawn")]
fn supervisor_spawn(
    manifest: String,
    init_state: Option<Value>,
    wasm_bytes: Option<Vec<u8>>,
) -> Result<String, String>;

#[import(module = "theater:simple/supervisor", name = "stop-child")]
fn supervisor_stop_child(child_id: String) -> Result<(), String>;

#[import(module = "theater:simple/store", name = "store-at-label")]
fn store_store_at_label(store_id: String, label: String, content: Vec<u8>) -> Result<String, String>;

// Plain-TCP listen address. TLS termination is configured separately
// via [handler.server_tls] in the sentinel-rendered manifest — see
// sentinel/ui-acceptor.template.toml. Local dev uses :8080 plaintext.
const LISTEN_ADDR: &str = "0.0.0.0:8080";

// Shared store used by inbox-acceptor; UI piggy-backs so we can read
// existing mailbox/message data without a parallel store.
const STORE_ID: &str = "inbox";
const BEARER_TOKEN_LABEL: &str = "ui-api-bearer-token";
const BASIC_AUTH_LABEL: &str = "ui-basic-auth";

#[derive(Deserialize)]
struct Config {
    bearer_token: String,
    basic_auth: String,
    api_base_url: String,
    ui_handler_manifest: String,
}

#[export(name = "theater:simple/actor.init")]
fn init(state: Value) -> Result<(AcceptorState, ()), String> {
    log(String::from("[ui-acceptor] init"));

    let raw = match state {
        Value::String(s) if !s.is_empty() => s,
        _ => {
            return Err(String::from(
                "ui-acceptor needs initial_state as a non-empty JSON string",
            ))
        }
    };

    let cfg: Config = serde_json::from_str(&raw)
        .map_err(|e| format!("initial_state is not valid JSON Config: {}", e))?;
    if cfg.bearer_token.is_empty()
        || cfg.basic_auth.is_empty()
        || cfg.api_base_url.is_empty()
        || cfg.ui_handler_manifest.is_empty()
    {
        return Err(String::from(
            "all four config fields (bearer_token, basic_auth, api_base_url, ui_handler_manifest) must be non-empty",
        ));
    }

    store_store_at_label(
        String::from(STORE_ID),
        String::from(BEARER_TOKEN_LABEL),
        cfg.bearer_token.into_bytes(),
    )
    .map_err(|e| format!("persist bearer token failed: {}", e))?;
    store_store_at_label(
        String::from(STORE_ID),
        String::from(BASIC_AUTH_LABEL),
        cfg.basic_auth.into_bytes(),
    )
    .map_err(|e| format!("persist basic_auth failed: {}", e))?;

    let listener_id = tcp_listen(String::from(LISTEN_ADDR))
        .map_err(|e| format!("listen failed: {}", e))?;
    log(format!(
        "[ui-acceptor] HTTP listening on {} (id={})",
        LISTEN_ADDR, listener_id
    ));

    Ok((
        AcceptorState {
            listener_id,
            ui_handler_manifest: cfg.ui_handler_manifest,
            api_base_url: cfg.api_base_url,
        },
        (),
    ))
}

#[export(name = "theater:simple/tcp-client.handle-connection")]
fn handle_connection(
    state: AcceptorState,
    connection_id: String,
) -> Result<(AcceptorState, ()), String> {
    // Always return Ok regardless of what happens inside. A single failing
    // connection must not kill the acceptor: if it does, theater treats
    // the whole supervision tree as failed and the process exits.
    if let Err(e) = try_handle_connection(&state, &connection_id) {
        log(format!(
            "[ui-acceptor] handle-connection failed (conn={}): {}",
            connection_id, e
        ));
    }
    Ok((state, ()))
}

fn try_handle_connection(state: &AcceptorState, connection_id: &str) -> Result<(), String> {
    let handler_id = supervisor_spawn(
        state.ui_handler_manifest.clone(),
        Some(Value::String(state.api_base_url.clone())),
        None,
    )
    .map_err(|e| format!("spawn ui-handler failed: {}", e))?;

    if let Err(e) = tcp_transfer(connection_id.to_string(), handler_id.clone()) {
        let _ = supervisor_stop_child(handler_id);
        return Err(format!("transfer failed: {}", e));
    }
    Ok(())
}
