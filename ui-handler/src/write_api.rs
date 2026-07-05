//! Outbound HTTP(S) to the inbox API. Shared by the write path
//! (`POST /v1/mailboxes/{from}/send`) and the read path (`GET …`).
//!
//! Per DESIGN.md §3: the bearer token never leaves the actor sandbox
//! — it's pulled from the store at init and attached as
//! `Authorization: Bearer <token>`.

use crate::request::url_encode;
use crate::{log, tcp_close, tcp_connect, tcp_receive, tcp_send, HandlerState};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Serialize;

#[derive(Serialize)]
pub struct SendRequest<'a> {
    pub to: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<&'a str>,
    pub subject: &'a str,
    pub body: &'a str,
}

pub struct ApiResponse {
    pub status: u16,
    pub body: String,
}

pub fn send_mail(
    state: &HandlerState,
    from: &str,
    req: &SendRequest<'_>,
) -> Result<ApiResponse, String> {
    let body = serde_json::to_string(req).map_err(|e| format!("serialize SendRequest: {}", e))?;
    let path = format!("/v1/mailboxes/{}/send", url_encode(from));
    api_post(state, &path, &body)
}

pub fn api_get(state: &HandlerState, path: &str) -> Result<ApiResponse, String> {
    api_request(state, "GET", path, None)
}

pub fn api_post(state: &HandlerState, path: &str, body: &str) -> Result<ApiResponse, String> {
    api_request(state, "POST", path, Some(body))
}

fn api_request(
    state: &HandlerState,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<ApiResponse, String> {
    // TLS is handled by the tcp handler's `[handler.client_tls]` config
    // (auto_handshake, on by default): `connect()` returns an already-
    // encrypted stream. We do NOT call `upgrade-to-tls-client` — with
    // client_tls enabled that path errors with "already TLS". This mirrors
    // the inbox CLI's outbound pattern (connect only, no explicit upgrade).
    // `use_tls` from the scheme only selected the default port above.
    let (host, port, _use_tls) = parse_base_url(&state.api_base_url)?;
    let conn = tcp_connect(format!("{}:{}", host, port))
        .map_err(|e| format!("tcp_connect {}:{}: {}", host, port, e))?;

    let body_str = body.unwrap_or("");
    let content_headers = if body.is_some() {
        format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body_str.len()
        )
    } else {
        String::new()
    };

    let req = format!(
        "{method} {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Authorization: Bearer {token}\r\n\
         {content_headers}\
         Connection: close\r\n\
         \r\n{body}",
        method = method,
        path = path,
        host = host,
        token = state.api_bearer_token,
        content_headers = content_headers,
        body = body_str,
    );
    tcp_send(conn.clone(), req.into_bytes()).map_err(|e| format!("send: {}", e))?;

    // Pull until peer closes or our buffer fills. v0 responses are small.
    let mut accumulated = Vec::new();
    loop {
        match tcp_receive(conn.clone(), 8192) {
            Ok(chunk) if chunk.is_empty() => break,
            Ok(chunk) => accumulated.extend_from_slice(&chunk),
            Err(e) => {
                // Many TLS peers close without close_notify; treat any
                // recv error after we've gotten bytes as end-of-stream.
                if accumulated.is_empty() {
                    let _ = tcp_close(conn);
                    return Err(format!("recv: {}", e));
                }
                log(format!("[ui-handler] api recv: {} (treating as eof)", e));
                break;
            }
        }
        if accumulated.len() >= 65536 {
            break;
        }
    }
    let _ = tcp_close(conn);

    parse_http_response(&accumulated)
}

fn parse_http_response(bytes: &[u8]) -> Result<ApiResponse, String> {
    let text = core::str::from_utf8(bytes).map_err(|_| String::from("non-utf8 api response"))?;
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| String::from("malformed api response"))?;
    let status_line = head.lines().next().unwrap_or("");
    let status: u16 = status_line
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("can't parse status from {:?}", status_line))?;
    Ok(ApiResponse {
        status,
        body: body.to_string(),
    })
}

fn parse_base_url(url: &str) -> Result<(String, u16, bool), String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| format!("api_base_url missing scheme: {}", url))?;
    let use_tls = match scheme {
        "https" => true,
        "http" => false,
        other => return Err(format!("unsupported scheme {}", other)),
    };
    // Strip any path suffix — we only care about authority.
    let authority = rest.split('/').next().unwrap_or(rest);
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse().map_err(|_| format!("bad port in {}", url))?,
        ),
        None => (authority.to_string(), if use_tls { 443 } else { 80 }),
    };
    Ok((host, port, use_tls))
}
