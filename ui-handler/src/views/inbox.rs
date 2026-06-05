use crate::request::url_encode;
use crate::{render, store_reads, HandlerState};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

pub fn render(addr: &str, state: &HandlerState) -> Vec<u8> {
    let messages = match store_reads::list_inbox(state, addr) {
        Ok(m) => m,
        Err(e) => return render::error(500, &format!("list inbox {}: {}", addr, e)),
    };

    let mut rows = String::new();
    if messages.is_empty() {
        rows.push_str("<li class=\"empty\">no messages</li>");
    } else {
        for m in messages {
            rows.push_str(&format!(
                "<li><a href=\"/m/{href_addr}/{href_id}\">\
                 <span class=\"from\">{from}</span>\
                 <span class=\"subject\">{subject}</span>\
                 </a></li>",
                href_addr = url_encode(addr),
                href_id = url_encode(&m.id),
                from = render::escape(&m.from),
                subject = render::escape(&m.subject),
            ));
        }
    }

    let content = format!(
        "<h1>{addr}</h1>\
         <p class=\"actions\"><a href=\"/compose?from={addr_q}\">compose from this address</a></p>\
         <ul class=\"inbox\">{rows}</ul>",
        addr = render::escape(addr),
        addr_q = url_encode(addr),
        rows = rows,
    );
    render::ok_html(render::shell(addr, &content))
}
