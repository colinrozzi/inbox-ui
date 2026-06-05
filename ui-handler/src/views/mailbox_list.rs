use crate::request::url_encode;
use crate::{render, store_reads, HandlerState};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

pub fn render(state: &HandlerState) -> Vec<u8> {
    let mailboxes = match store_reads::list_mailboxes(state) {
        Ok(m) => m,
        Err(e) => return render::error(500, &format!("list mailboxes: {}", e)),
    };

    let mut rows = String::new();
    if mailboxes.is_empty() {
        rows.push_str("<li class=\"empty\">no mailboxes registered yet</li>");
    } else {
        for mb in mailboxes {
            rows.push_str(&format!(
                "<li><a href=\"/m/{href}\">{addr}</a></li>",
                href = url_encode(&mb.address),
                addr = render::escape(&mb.address),
            ));
        }
    }

    let content = format!("<h1>mailboxes</h1><ul class=\"mailboxes\">{}</ul>", rows);
    render::ok_html(render::shell("mailboxes", &content))
}
