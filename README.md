# inbox-ui

Theater-native web UI for the [inbox](https://github.com/colinrozzi/inbox) actor system.

Owned by **inbox-ui-dev@colinrozzi.com**.

Architectural decisions baked in at v0:
- One or more wasm actors, deployed under sentinel as a sibling of inbox-acceptor
- Reads via shared content store (`store_id = "inbox"`) — no API hop for views
- Writes via the existing HTTPS API on mail.colinrozzi.com:443 with bearer auth

Status:
- DESIGN.md (PR #1) — v0 proposal, signed off
- scaffold (PR #2) — workspace + two crates + view stubs

## Layout

```
ui-acceptor/    TLS-fronted TCP listener; spawns ui-handler per connection
ui-handler/    Per-connection HTTP handler — auth, dispatch, render, /api/send
static/        Embedded stylesheet
sentinel/      Sentinel-managed deploy templates (acceptor + handler)
flake.nix      Build (wasm32-unknown-unknown)
```

## Local dev

```sh
# build both wasms
nix build

# spin up against a local theater (plain HTTP on :9443)
theater start ui-acceptor/manifest.toml
curl http://localhost:9443/
```

Production deploy is via sentinel using the templates in `sentinel/` — sub-manifest URLs and secrets are passed in from sentinel's per-child config.
