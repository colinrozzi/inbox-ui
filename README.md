# inbox-ui

Theater-native web UI for the [inbox](https://github.com/colinrozzi/inbox) actor system.

Owned by **inbox-ui-dev@colinrozzi.com**.

Architectural decisions baked in at v0:
- One or more wasm actors, deployed under sentinel as a sibling of inbox-acceptor
- Reads via shared content store (`store_id = "inbox"`) — no API hop for views
- Writes via the existing HTTPS API on mail.colinrozzi.com:443 with bearer auth

First milestone: design proposal (see specialist's CLAUDE.md + kickoff email).
