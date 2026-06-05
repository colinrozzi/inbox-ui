use crate::request::{url_encode, Request};
use crate::write_api::SendRequest;
use crate::{render, write_api, HandlerState};
use alloc::format;
use alloc::vec::Vec;

pub fn render(req: &Request) -> Vec<u8> {
    let from = req.query.get("from").cloned().unwrap_or_default();
    let to = req.query.get("to").cloned().unwrap_or_default();
    let subject = req.query.get("subject").cloned().unwrap_or_default();
    let body = req.query.get("body").cloned().unwrap_or_default();

    let content = format!(
        "<h1>compose</h1>\
         <form method=\"post\" action=\"/send\" class=\"compose\">\
           <label>from <input name=\"from\" value=\"{from}\" required></label>\
           <label>to   <input name=\"to\"   value=\"{to}\" required></label>\
           <label>cc   <input name=\"cc\"   value=\"\"></label>\
           <label>subject <input name=\"subject\" value=\"{subject}\"></label>\
           <label>body <textarea name=\"body\" rows=\"15\">{body}</textarea></label>\
           <button type=\"submit\">send</button>\
         </form>",
        from = render::escape(&from),
        to = render::escape(&to),
        subject = render::escape(&subject),
        body = render::escape(&body),
    );
    render::ok_html(render::shell("compose", &content))
}

pub fn submit(req: &Request, state: &HandlerState) -> Vec<u8> {
    let form = req.form();
    let from = form.get("from").cloned().unwrap_or_default();
    let to = form.get("to").cloned().unwrap_or_default();
    let cc = form.get("cc").cloned().unwrap_or_default();
    let subject = form.get("subject").cloned().unwrap_or_default();
    let body = form.get("body").cloned().unwrap_or_default();

    if from.is_empty() || to.is_empty() {
        return render::error(400, "from and to are required");
    }

    let to_list: Vec<&str> = to.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    let cc_list: Vec<&str> = cc.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();

    let send_req = SendRequest {
        to: to_list,
        cc: cc_list,
        subject: &subject,
        body: &body,
    };

    match write_api::send_mail(state, &from, &send_req) {
        Ok(resp) if resp.status >= 200 && resp.status < 300 => {
            render::redirect(&format!("/m/{}", url_encode(&from)))
        }
        Ok(resp) => render::error(
            502,
            &format!("api returned {}: {}", resp.status, resp.body),
        ),
        Err(e) => render::error(502, &format!("send failed: {}", e)),
    }
}
