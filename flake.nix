{
  description = "inbox-ui: web UI for the inbox mail service, built on Theater";

  # packr 0.11.0 plain-build model: an actor is a plain `cargo build` cdylib.
  # setup_guest!() links dlmalloc in, so the wasm exports its own growable
  # memory + __pack_alloc/__pack_free + lifecycle and imports only host
  # theater:simple/*. No compose step, no binaryen, no fixed-base recipe — the
  # two link-args live in each actor's .cargo/config.toml. The devShell just
  # needs a wasm32 rust toolchain + wasm-tools (import-surface verify).

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";

    # Pinned to the packr-0.11.0 theater (PR #149, rev 73a4540b). Used for the
    # runtime binary (local `theater spawn` / `nix build .#theater`); the actor
    # BUILD no longer needs theater.
    theater = {
      url = "github:colinrozzi/theater/73a4540b";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.crane.follows = "crane";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane, theater }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
        };

      in {
        # Build + verify shell (fast — no theater/binaryen build):
        #   nix develop --command bash -c 'cd ui-acceptor && cargo build --target wasm32-unknown-unknown --release'
        #   nix develop --command bash -c 'cd ui-handler  && cargo build --target wasm32-unknown-unknown --release'
        #   wasm-tools print <name>.wasm | grep '(import' | grep -v theater:simple/   # must be empty
        devShells.default = pkgs.mkShell {
          packages = [ rustToolchain pkgs.wasm-tools ];
          # stderr so `nix develop --command wasm-tools print` stdout stays clean.
          shellHook = ''
            {
              echo "inbox-ui dev environment (packr 0.11.0 plain build)"
              echo "  (cd ui-acceptor && cargo build --target wasm32-unknown-unknown --release)"
              echo "  (cd ui-handler  && cargo build --target wasm32-unknown-unknown --release)"
            } >&2
          '';
        };

        # nix build .#theater — the pinned 0.11.0 theater runtime binary (spawn).
        packages.theater = theater.packages.${system}.default;
      });
}
