//! HTTP Basic auth check. Single shared credential, set via
//! ui-acceptor's initial_state and persisted to the shared store under
//! the BASIC_AUTH_LABEL. Per DESIGN.md §0 this is the v0 multi-user
//! story: replace before a second user exists.

use crate::request::Request;
use alloc::string::String;
use base64::{engine::general_purpose::STANDARD, Engine as _};

pub fn check(req: &Request, expected: &str) -> bool {
    let header = match req.header("authorization") {
        Some(h) => h,
        None => return false,
    };
    let encoded = match header.strip_prefix("Basic ") {
        Some(s) => s.trim(),
        None => return false,
    };
    let decoded_bytes = match STANDARD.decode(encoded) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let decoded = match String::from_utf8(decoded_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };
    constant_time_eq(decoded.as_bytes(), expected.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
