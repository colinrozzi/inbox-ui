use crate::request::url_encode;
use crate::{render, store_reads, HandlerState};
use alloc::format;
use alloc::vec::Vec;

pub fn render(addr: &str, id: &str, state: &HandlerState) -> Vec<u8> {
    let msg = match store_reads::get_message(state, addr, id) {
        Ok(m) => m,
        Err(e) => return render::error(500, &format!("get message {}/{}: {}", addr, id, e)),
    };

    let content = format!(
        "<article class=\"message\">\
         <h1>{subject}</h1>\
         <dl class=\"headers\">\
           <dt>from</dt><dd>{from}</dd>\
           <dt>to</dt><dd>{to}</dd>\
           <dt>received</dt><dd>{received}</dd>\
         </dl>\
         <pre class=\"body\">{body}</pre>\
         <p class=\"actions\"><a href=\"/compose?from={addr_q}&to={from_q}&subject={reply_subject}\">reply</a></p>\
         </article>",
        subject = render::escape(&msg.subject),
        from = render::escape(&msg.from),
        to = render::escape(&msg.to),
        received = msg.received_at,
        body = render::escape(&msg.body),
        addr_q = url_encode(addr),
        from_q = url_encode(&msg.from),
        reply_subject = url_encode(&format!("Re: {}", msg.subject)),
    );
    render::ok_html(render::shell(&msg.subject, &content))
}
