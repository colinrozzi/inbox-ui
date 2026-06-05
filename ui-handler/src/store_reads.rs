//! Direct-store read path — STUB.
//!
//! Per DESIGN.md §3 the v0 proposal was direct store reads, but a
//! subsequent open question (see manager email "you and ticket-ui-dev
//! edit DESIGN.md §3 consistently") flips this to API-over-loopback. We
//! don't fill in either body until that's decided so we don't ship code
//! that immediately gets rewritten.
//!
//! The signature shape below is independent of read-path choice — the
//! views call these and don't care whether they hit the store directly
//! or go via HTTP to /api/messages on loopback.

use crate::HandlerState;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone)]
pub struct MailboxSummary {
    pub address: String,
}

// Fields below are read once views land — `allow(dead_code)` keeps the
// scaffold building clean until the read path is wired.
#[allow(dead_code)]
#[derive(Clone)]
pub struct InboxMessage {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub received_at: u64,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct MessageFull {
    pub id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub received_at: u64,
}

pub fn list_mailboxes(_state: &HandlerState) -> Result<Vec<MailboxSummary>, String> {
    Err(String::from("read path not yet wired — see DESIGN.md §3"))
}

pub fn list_inbox(_state: &HandlerState, _addr: &str) -> Result<Vec<InboxMessage>, String> {
    Err(String::from("read path not yet wired — see DESIGN.md §3"))
}

pub fn get_message(_state: &HandlerState, _addr: &str, _id: &str) -> Result<MessageFull, String> {
    Err(String::from("read path not yet wired — see DESIGN.md §3"))
}
