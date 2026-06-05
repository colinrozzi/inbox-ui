//! HTTP response builders + tiny HTML helpers.
//!
//! No templating engine: each view function returns a `String` of HTML
//! built with `format!`. This keeps the dep budget at zero for rendering
//! and means the entire UI is searchable as raw text.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

pub fn ok_html(body: String) -> Vec<u8> {
    http(200, "text/html; charset=utf-8", body.into_bytes(), &[])
}

pub fn css(body: &'static str) -> Vec<u8> {
    http(200, "text/css; charset=utf-8", body.as_bytes().to_vec(), &[])
}

pub fn error(status: u16, msg: &str) -> Vec<u8> {
    let body = shell("error", &format!("<p class=\"err\">{}</p>", escape(msg)));
    http(status, "text/html; charset=utf-8", body.into_bytes(), &[])
}

pub fn redirect(location: &str) -> Vec<u8> {
    http(303, "text/plain", b"".to_vec(), &[("Location", location)])
}

pub fn basic_auth_challenge() -> Vec<u8> {
    http(
        401,
        "text/html; charset=utf-8",
        shell("auth required", "<p>auth required</p>").into_bytes(),
        &[("WWW-Authenticate", "Basic realm=\"inbox-ui\"")],
    )
}

fn http(status: u16, content_type: &str, body: Vec<u8>, extra: &[(&str, &str)]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        303 => "See Other",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        status,
        reason,
        content_type,
        body.len()
    );
    for (k, v) in extra {
        head.push_str(&format!("{}: {}\r\n", k, v));
    }
    head.push_str("\r\n");
    let mut out = head.into_bytes();
    out.extend_from_slice(&body);
    out
}

/// Page shell — header, nav, content, footer. Every view wraps its
/// content in this so the chrome stays consistent.
pub fn shell(title: &str, content: &str) -> String {
    format!(
        "<!doctype html>\n\
<html lang=\"en\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
  <title>{title} · inbox</title>\n\
  <link rel=\"stylesheet\" href=\"/static/style.css\">\n\
</head>\n\
<body>\n\
<header><a href=\"/\">inbox</a> · <a href=\"/compose\">compose</a></header>\n\
<main>{content}</main>\n\
</body>\n\
</html>\n",
        title = escape(title),
        content = content,
    )
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}
