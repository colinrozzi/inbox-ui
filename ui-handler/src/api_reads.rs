//! Read path: list mailboxes / inbox / single message via the inbox API.
//!
//! Per DESIGN.md §3 (post-2026-06-05 flip) reads go over the same
//! `${api_base_url}` + bearer-token path that writes do. There is no
//! dedicated single-message endpoint, so `get_message` fetches the
//! inbox listing for the address and filters to the requested id.

use crate::request::url_encode;
use crate::write_api::api_get;
use crate::HandlerState;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Deserialize;

#[derive(Clone)]
#[allow(dead_code)]
pub struct MailboxSummary {
    pub address: String,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct InboxMessage {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub received_at: u64,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct MessageFull {
    pub id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub received_at: u64,
}

#[derive(Deserialize)]
struct MailboxListResp {
    mailboxes: Vec<String>,
}

#[derive(Deserialize)]
struct ApiInboxMessage {
    id: u64,
    from: String,
    to: String,
    subject: String,
    body: String,
    received_at: u64,
}

#[derive(Deserialize)]
struct InboxPageResp {
    messages: Vec<ApiInboxMessage>,
    // `next_cursor` is present in the API response; v0 doesn't paginate so we drop it.
}

pub fn list_mailboxes(state: &HandlerState) -> Result<Vec<MailboxSummary>, String> {
    let resp = api_get(state, "/v1/mailboxes")?;
    if resp.status != 200 {
        return Err(format!("api returned {}: {}", resp.status, resp.body));
    }
    let parsed: MailboxListResp =
        serde_json::from_str(&resp.body).map_err(|e| format!("parse mailboxes: {}", e))?;
    Ok(parsed
        .mailboxes
        .into_iter()
        .map(|address| MailboxSummary { address })
        .collect())
}

pub fn list_inbox(state: &HandlerState, addr: &str) -> Result<Vec<InboxMessage>, String> {
    let page = fetch_inbox(state, addr)?;
    Ok(page
        .messages
        .into_iter()
        .map(|m| InboxMessage {
            id: m.id.to_string(),
            from: m.from,
            subject: m.subject,
            received_at: m.received_at,
        })
        .collect())
}

pub fn get_message(state: &HandlerState, addr: &str, id: &str) -> Result<MessageFull, String> {
    let want_id: u64 = id
        .parse()
        .map_err(|_| format!("invalid message id (not a u64): {}", id))?;
    let page = fetch_inbox(state, addr)?;
    page.messages
        .into_iter()
        .find(|m| m.id == want_id)
        .map(|m| MessageFull {
            id: m.id.to_string(),
            from: m.from,
            to: m.to,
            subject: m.subject,
            body: m.body,
            received_at: m.received_at,
        })
        .ok_or_else(|| format!("message {} not found in {}", id, addr))
}

fn fetch_inbox(state: &HandlerState, addr: &str) -> Result<InboxPageResp, String> {
    let path = format!("/v1/mailboxes/{}/inbox", url_encode(addr));
    let resp = api_get(state, &path)?;
    if resp.status != 200 {
        return Err(format!("api returned {}: {}", resp.status, resp.body));
    }
    serde_json::from_str(&resp.body).map_err(|e| format!("parse inbox: {}", e))
}
