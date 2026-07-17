{
  description = "inbox-ui: web UI for the inbox mail service, built on Theater";

  # packr 0.10.2 self-contained model: actors are built with
  # `theater build --release <actor-dir>`, which cargo-builds the fixed-base
  # member (see each actor's .cargo/config.toml), links it with the packr
  # bundled allocator into a self-contained <name>.composite.wasm, and
  # verifies host-only imports. There is no crane/nix build of the wasm here;
  # the devShell provides the toolchain and CI runs `theater build` in it.

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";

    # Pinned to the canonical packr-0.10.2 theater rev 7daab2ad (theater
    # PR #141: `theater build`/`theater compose` + the 0.10.x self-contained
    # loader). The fleet-canonical pin; bump in lockstep with the runtime.
    theater = {
      url = "github:colinrozzi/theater/7daab2ad";
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

        # theater CLI (has `theater build` + the 0.10.x runtime).
        theaterBin = theater.packages.${system}.default;

      in {
        # `theater build` needs wasm-merge (binaryen) + wasm-tools on PATH, plus
        # cargo with the wasm32 target. This shell provides all of them:
        #   nix develop --command theater build --release ui-acceptor
        #   nix develop --command theater build --release ui-handler
        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            theaterBin
            pkgs.binaryen   # wasm-merge — packr::link fuses the composite
            pkgs.wasm-tools # validate + self-contained import check
          ];
          # Echo to STDERR so `nix develop --command <cmd>` leaves stdout clean
          # (the release CI captures `wasm-tools print` stdout to grep imports).
          shellHook = ''
            {
              echo "inbox-ui dev environment (packr 0.10.2 self-contained)"
              echo "  theater build --release ui-acceptor   # -> ui_acceptor.composite.wasm"
              echo "  theater build --release ui-handler    # -> ui_handler.composite.wasm"
              echo "  theater spawn ui-acceptor/manifest.toml"
            } >&2
          '';
        };

        # nix build .#theater — the pinned theater CLI/runtime binary.
        packages.theater = theaterBin;
      });
}
