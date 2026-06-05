# inbox-ui — v0 design

Author: inbox-ui-dev@colinrozzi.com
Status: proposal — awaiting sign-off from manager + Colin before any code lands.

This document picks the shape of the v0 UI. It is deliberately opinionated and small. Anything not called out here is out of scope until v1.

## TL;DR

- Two-actor split mirroring inbox: **ui-acceptor** (TLS listener) + **ui-handler** (per-connection).
- Own TLS endpoint on `:8443`, shared Let's Encrypt cert mounted from the VPS — no dependency on inbox-acceptor's route table.
- Reads go straight to the shared `theater:simple/store` (`store_id = "inbox"`). Writes go to the existing API on `mail.colinrozzi.com:443` over loopback HTTPS with the bearer token.
- Server-rendered HTML, no SPA, no JS framework. One stylesheet. Full-page navigation.
- HTTP Basic auth in front of every route. Single shared credential from env. Good enough for v0; replace before we ever invite a second user.
- Four screens: mailbox list, per-mailbox inbox, single message, compose.

## 1. View inventory

| Path                | Screen              | Notes                                            |
|---------------------|---------------------|--------------------------------------------------|
| `GET /`             | Mailbox list        | All registered addresses, latest-message preview |
| `GET /m/{addr}`     | Inbox for mailbox   | Reverse-chronological list, capped at 50         |
| `GET /m/{addr}/{id}` | Single message     | Headers + body, "reply" link prefilling compose  |
| `GET /compose`      | Compose form        | `from`, `to`, `cc`, `subject`, `body`            |
| `POST /send`        | Submit compose      | Calls API, then 303 → `/m/{from}`                |
| `GET /static/*`     | Stylesheet, favicon | One file each, served from embedded bytes        |

Errors render as the same shell with an inline banner; no separate error pages.

## 2. Listener strategy

**Decision: separate TLS endpoint on `:8443`, owned by ui-acceptor.**

Two options were on the table:

- **(A) Separate port, own TLS** *(chosen).* ui-acceptor binds `:8443`, terminates TLS using the same Let's Encrypt cert that inbox-acceptor uses (mounted via the sentinel manifest). UI ships independently of the API.
- **(B) Sub-route of inbox-acceptor's `:443`.** Adds a routing prefix (e.g. `/ui/*`) inside inbox-acceptor that dispatches to ui-handler. One DNS name, one cert, but couples UI changes to inbox-acceptor deploys and requires inbox-dev work to land first.

Why (A):

- Zero cross-actor work to ship v0 — inbox-dev's queue stays clear.
- Lifecycle independence: redeploying the UI doesn't touch the API path; a crashlooping UI can't degrade `mail.colinrozzi.com`.
- Same operational shape as inbox-acceptor itself (TLS listen socket → per-conn handler), so the existing patterns transfer 1:1.

Tradeoffs accepted:

- A second cert mount point in the sentinel manifest. Cert lives on a shared volume already; nothing new to provision.
- A new DNS record or non-standard port. Going with `mail.colinrozzi.com:8443` initially to avoid a DNS change; we can move to `inbox.colinrozzi.com:443` once we want a clean URL.
- TLS termination duplicated in code. Acceptable: it's a few dozen lines and we already have a reference.

If (B) turns out cheaper than expected, the ui-handler stays the same — only the front door changes. Cost of reversal is low.

## 3. Wire shape per view

Per the 2026-06-05 flip (matching ticket-ui's same decision): the API is the public contract; the store is an implementation detail we don't depend on. Reads AND writes both go through the inbox API.

| View              | Read source                                       | Write target                                       |
|-------------------|---------------------------------------------------|----------------------------------------------------|
| Mailbox list      | API: `GET /v1/mailboxes`                          | —                                                  |
| Inbox for mailbox | API: `GET /v1/mailboxes/{addr}/inbox`             | —                                                  |
| Single message    | API: `GET /v1/mailboxes/{addr}/inbox` + filter id | —                                                  |
| Compose (render)  | — (static form)                                   | —                                                  |
| Compose (submit)  | —                                                 | API: `POST /v1/mailboxes/{from}/send`              |

All API calls go to `${api_base_url}` (production: `https://mail.colinrozzi.com:443`) via `theater:simple/tcp.connect` + `upgrade-to-tls-client`. The bearer token is shared between reads and writes (one shared store label, written by ui-acceptor at init).

Rules:

- **Reads and writes both go through the inbox API.** The API is the public contract; the store is an implementation detail. Schema changes can land backend-only without touching the UI.
- **One bearer token, server-side.** ui-handler reads the bearer from the shared store at its own init; ui-acceptor wrote it there from `initial_state`. The browser never sees it.
- **No JS-driven fetches in v0.** Every state change is a form POST returning a redirect. HTMX-style partials can come later; the cost of going from full-page to partials is small, the cost of going from SPA back to forms is large.
- **Single-message reads piggyback on the inbox listing.** No dedicated `GET /v1/mailboxes/{addr}/inbox/{id}` endpoint exists; we fetch the inbox page (which includes message bodies) and filter to the requested id. Extra body-fetch per single-message page load is acceptable until the inbox page grows large enough that paginating matters.

## 4. Actor decomposition

**Decision: mirror inbox.** Two actors:

- **ui-acceptor**: owns the TLS listen socket on `:8443`. On every accepted connection, spawns a fresh **ui-handler** child and hands it the TLS stream. No request parsing.
- **ui-handler**: parses the HTTP request, authenticates (Basic), reads from store or calls API, renders HTML, writes response, exits. One handler per connection. Stateless across requests.

Why not one monolithic actor:

- Inbox already runs this shape in production; the operational and debugging muscle memory carries over.
- A handler that panics or hangs can't poison the listener.
- Per-conn isolation gives us a place to attach per-request tracing later without changing the listener.

Why not finer-grained splits (e.g. one actor per view):

- Premature. The handler is the dispatch boundary; adding more actors just adds spawn cost and message hops.

## 5. Framework / build

**HTML**: server-rendered via `format!` macros in Rust. No templating engine in v0. Each view is a single function returning a `String`. If templating churn becomes painful we add `askama` later — but four views don't justify a dep.

**CSS**: one hand-written stylesheet, ~150–250 lines, embedded in the wasm via `include_str!` and served at `/static/style.css`. No framework, no preprocessor. Aim for legibility, not visual ambition.

**JS**: none in v0. If a single feature genuinely needs it (e.g. auto-focus on compose), inline `<script>` tag in the page, not a separate file.

**Build**: nix flake mirroring inbox's. Cargo workspace with `ui-acceptor` and `ui-handler` member crates. `nix build` produces two `.wasm` artifacts and the sub-manifest TOMLs. Release tag format matches inbox: `release-YYYYMMDD-<sha>` carrying both wasms + manifests as assets.

**Deps budget** (initial):
- `packr-guest` (mandatory, theater guest runtime — see memory `inbox_deps_no_theater_crate`)
- `httparse` or hand-rolled for request parsing (one or the other, not both)
- `serde` + `serde_json` for the `/api/send` payload
- `base64` for Basic auth decode

No web framework, no async runtime beyond what theater provides, no logging crate beyond `eprintln!`.

## 6. Out of scope for v0

Explicit cuts, in priority order of "things people will ask about":

- **Search.** Store doesn't index; we'd need a separate path. Out.
- **Attachments.** API doesn't expose them yet. Out.
- **Real-time updates.** No SSE, no WebSocket, no polling. Refresh the page. Out.
- **Threading / conversation view.** Messages are flat. Out.
- **Drafts.** Compose is fire-and-forget; closing the tab loses the draft. Out.
- **Pagination.** Hard cap at 50 most recent messages per mailbox. Out.
- **Read/unread state.** Store is append-only; tracking this needs a side channel. Out.
- **Multi-user.** Single shared HTTP Basic credential. Out.
- **Mobile-first styling.** Should not be broken on a phone, but desktop is the target.
- **Accessibility audit.** Use semantic HTML; defer formal audit.

Anything on this list re-enters scope only when there is a concrete user-facing reason; not before.

## 7. Open questions for inbox-dev / manager

Before any handler code is written:

1. ~~**Store key layout.** What is the actual prefix/key shape for `mailboxes/` and `messages/{addr}/`?~~ — **Moot** as of the §3 flip. The API insulates us from the store schema.
2. **API base URL from inside an actor.** Do ui-handler actors reach `mail.colinrozzi.com:443` over the public internet, or is there an internal hostname / loopback shortcut sentinel exposes? Production answer is the public URL via the same Let's Encrypt cert; "loopback" is the conceptual category, not a specific endpoint. Revisit if frontdoor exposes an internal-only entry.
3. **TLS cert path inside the actor sandbox.** Where does sentinel mount the Let's Encrypt fullchain + key for the new actor? Same path as inbox-acceptor, or does sentinel-dev need to add a mount?
4. **DNS / port.** OK to use `mail.colinrozzi.com:8443` for v0, or does Colin want a new subdomain provisioned up-front? — Probably moot once frontdoor SNI-routes; revisit then.

Answers to 1–3 don't change the design; they just unblock implementation. Answer to 4 is a Colin call.

## 8. Path to v1

Once v0 is live and we've used it for a week, the most likely next steps in rough order:

1. Reply (compose pre-fills `to`, `subject`, body quote).
2. Mark-as-read state, stored in a separate per-user store namespace.
3. Real session auth (cookie + per-user credential).
4. HTMX-style partials for compose and message-open to cut the full-page reload.
5. Search over the store (likely needs a backend index, so coordinate with inbox-dev).

None of these block v0.
