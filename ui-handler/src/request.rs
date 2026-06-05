//! Minimal HTTP/1.1 request parser. v0 handles GET + POST with
//! `application/x-www-form-urlencoded` bodies; that's all four views
//! need. No multipart, no chunked, no Expect: 100-continue.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub struct Request {
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

impl Request {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = core::str::from_utf8(bytes).map_err(|_| String::from("non-utf8 request"))?;
        let (head, body) = text
            .split_once("\r\n\r\n")
            .ok_or_else(|| String::from("missing header/body separator"))?;

        let mut lines = head.split("\r\n");
        let first = lines.next().ok_or_else(|| String::from("empty request"))?;
        let mut parts = first.split(' ');
        let method = parts.next().unwrap_or("").to_string();
        let path_and_query = parts.next().unwrap_or("/");
        let (path_raw, query_str) = match path_and_query.find('?') {
            Some(i) => (&path_and_query[..i], &path_and_query[i + 1..]),
            None => (path_and_query, ""),
        };

        let mut headers = BTreeMap::new();
        for line in lines {
            if let Some(i) = line.find(':') {
                let name = line[..i].trim().to_ascii_lowercase();
                let value = line[i + 1..].trim().to_string();
                headers.insert(name, value);
            }
        }

        Ok(Self {
            method,
            path: url_decode(path_raw),
            query: parse_form(query_str),
            headers,
            body: body.to_string(),
        })
    }

    pub fn form(&self) -> BTreeMap<String, String> {
        parse_form(&self.body)
    }

    pub fn header(&self, name: &str) -> Option<&String> {
        self.headers.get(&name.to_ascii_lowercase())
    }
}

pub fn parse_form(s: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for pair in s.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.find('=') {
            Some(i) => (&pair[..i], &pair[i + 1..]),
            None => (pair, ""),
        };
        out.insert(url_decode(k), url_decode(v));
    }
    out
}

pub fn url_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_digit(bytes[i + 1]);
                let lo = hex_digit(bytes[i + 2]);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push((h << 4) | l);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| format!("<invalid-utf8>"))
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
