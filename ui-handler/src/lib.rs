//! ui-handler: per-connection HTTP handler for the inbox UI.
//!
//! Receives one HTTP request (transferred from ui-acceptor), authenticates
//! via HTTP Basic, dispatches to a view renderer, writes the response,
//! closes the connection, and shuts itself down.
//!
//! Routes:
//!   GET  /                         → mailbox list
//!   GET  /m/<addr>                 → inbox for one mailbox
//!   GET  /m/<addr>/<id>            → single message
//!   GET  /compose                  → compose form
//!   POST /send                     → submit compose → API → 303 → /m/<from>
//!   GET  /static/style.css         → embedded stylesheet
//!
//! Reads (mailbox list, inbox listing, message body) are STUBBED in v0
//! pending the store-direct vs API-over-loopback decision (see DESIGN.md
//! §3). Once that lands, fill in store.rs accordingly.

#![no_std]
extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use packr_guest::{export, import, pack_types, GraphValue, Value};

packr_guest::setup_guest!();

mod api_reads;
mod auth;
mod render;
mod request;
mod views;
mod write_api;

#[derive(Clone, GraphValue)]
#[graph(crate = "packr_guest::composite_abi")]
pub struct HandlerState {
    pub api_base_url: String,
    pub api_bearer_token: String,
    pub basic_auth_credential: String,
}

pack_types! {
    imports {
        theater:simple/runtime {
            log: func(msg: string),
            shutdown: func(data: option<list<u8>>) -> result<_, string>,
        }
        theater:simple/tcp {
            connect: func(address: string) -> result<string, string>,
            receive: func(connection-id: string, max-bytes: u32) -> result<list<u8>, string>,
            send: func(connection-id: string, data: list<u8>) -> result<u64, string>,
            close: func(connection-id: string) -> result<_, string>,
            upgrade-to-tls-client: func(connection-id: string, server-name: string) -> result<_, string>,
        }
        theater:simple/store {
            get: func(store-id: string, content-ref: string) -> result<list<u8>, string>,
            get-by-label: func(store-id: string, label: string) -> result<option<string>, string>,
            list-labels: func(store-id: string) -> result<list<string>, string>,
        }
    }
    exports {
        theater:simple/actor.init: func(state: value) -> result<handler-state, string>,
        theater:simple/tcp-client.handle-connection-transfer: func(state: handler-state, connection-id: string) -> result<handler-state, string>,
    }
}

#[import(module = "theater:simple/runtime", name = "log")]
pub(crate) fn log(msg: String);

#[import(module = "theater:simple/runtime", name = "shutdown")]
fn shutdown(data: Option<Vec<u8>>) -> Result<(), String>;

#[import(module = "theater:simple/tcp", name = "connect")]
pub(crate) fn tcp_connect(address: String) -> Result<String, String>;

#[import(module = "theater:simple/tcp", name = "receive")]
pub(crate) fn tcp_receive(connection_id: String, max_bytes: u32) -> Result<Vec<u8>, String>;

#[import(module = "theater:simple/tcp", name = "send")]
pub(crate) fn tcp_send(connection_id: String, data: Vec<u8>) -> Result<u64, String>;

#[import(module = "theater:simple/tcp", name = "close")]
pub(crate) fn tcp_close(connection_id: String) -> Result<(), String>;

#[import(module = "theater:simple/tcp", name = "upgrade-to-tls-client")]
pub(crate) fn tcp_upgrade_to_tls_client(connection_id: String, server_name: String) -> Result<(), String>;

#[import(module = "theater:simple/store", name = "get")]
pub(crate) fn store_get(store_id: String, content_ref: String) -> Result<Vec<u8>, String>;

#[import(module = "theater:simple/store", name = "get-by-label")]
pub(crate) fn store_get_by_label(store_id: String, label: String) -> Result<Option<String>, String>;

#[import(module = "theater:simple/store", name = "list-labels")]
pub(crate) fn store_list_labels(store_id: String) -> Result<Vec<String>, String>;

const STORE_ID: &str = "inbox";
const BEARER_TOKEN_LABEL: &str = "ui-api-bearer-token";
const BASIC_AUTH_LABEL: &str = "ui-basic-auth";

fn load_label_as_string(label: &str) -> Result<String, String> {
    let content_ref = store_get_by_label(String::from(STORE_ID), String::from(label))
        .map_err(|e| format!("{} lookup failed: {}", label, e))?
        .ok_or_else(|| format!("{} label not set (acceptor should have written it)", label))?;
    let bytes = store_get(String::from(STORE_ID), content_ref)
        .map_err(|e| format!("{} get failed: {}", label, e))?;
    String::from_utf8(bytes).map_err(|_| format!("{} is not valid UTF-8", label))
}

#[export(name = "theater:simple/actor.init")]
fn init(state: Value) -> Result<(HandlerState, ()), String> {
    let api_base_url = match state {
        Value::String(s) => s,
        _ => return Err(String::from(
            "ui-handler init: expected init_state = string (api base url)",
        )),
    };
    let api_bearer_token = load_label_as_string(BEARER_TOKEN_LABEL)?;
    let basic_auth_credential = load_label_as_string(BASIC_AUTH_LABEL)?;
    Ok((
        HandlerState {
            api_base_url,
            api_bearer_token,
            basic_auth_credential,
        },
        (),
    ))
}

#[export(name = "theater:simple/tcp-client.handle-connection-transfer")]
fn handle_connection_transfer(
    state: HandlerState,
    connection_id: String,
) -> Result<(HandlerState, ()), String> {
    let request_bytes = tcp_receive(connection_id.clone(), 65536).unwrap_or_default();
    let response = dispatch(&request_bytes, &state);
    if let Err(e) = tcp_send(connection_id.clone(), response) {
        log(format!("[ui-handler] send failed: {}", e));
    }
    let _ = tcp_close(connection_id);
    let _ = shutdown(None);
    Ok((state, ()))
}

fn dispatch(request_bytes: &[u8], state: &HandlerState) -> Vec<u8> {
    let req = match request::Request::parse(request_bytes) {
        Ok(r) => r,
        Err(e) => return render::error(400, &format!("bad request: {}", e)),
    };

    // Static assets are served before auth so 401-redirected pages can
    // still load the stylesheet.
    if req.method == "GET" && req.path == "/static/style.css" {
        return render::css(views::STYLE_CSS);
    }

    if !auth::check(&req, &state.basic_auth_credential) {
        return render::basic_auth_challenge();
    }

    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => views::mailbox_list::render(state),
        ("GET", "/compose") => views::compose::render(&req),
        ("POST", "/send") => views::compose::submit(&req, state),
        (method, path) => {
            if let Some(rest) = path.strip_prefix("/m/") {
                match (method, rest.split('/').collect::<Vec<_>>().as_slice()) {
                    ("GET", [addr]) => views::inbox::render(addr, state),
                    ("GET", [addr, id]) => views::message::render(addr, id, state),
                    _ => render::error(404, "not found"),
                }
            } else {
                render::error(404, "not found")
            }
        }
    }
}
