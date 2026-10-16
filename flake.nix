{
  description = "polyoxide's dev shell: the toolchains and cargo tools CI installs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSystem =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );
    in
    {
      devShells = forEachSystem (
        pkgs:
        let
          # The removal gate's pin (.github/workflows/ci.yml), which CI's floating stable
          # matched when it was set; cargo-semver-checks reads only the rustdoc JSON some
          # toolchains emit, so the two move together.
          rust = pkgs.rust-bin.stable."1.99.0".minimal.override {
            extensions = [
              "clippy"
              "rustfmt"
            ];
          };
          # The MSRV job's toolchain. It sits behind `msrv-cargo`, so it never shadows the
          # default `cargo`, and it builds into its own target directory.
          msrv = pkgs.rust-bin.stable."1.91.0".minimal;
          msrv-cargo = pkgs.writeShellScriptBin "msrv-cargo" ''
            export RUSTC=${msrv}/bin/rustc RUSTDOC=${msrv}/bin/rustdoc
            export CARGO_TARGET_DIR="''${CARGO_TARGET_DIR:-target}/msrv"
            exec ${msrv}/bin/cargo "$@"
          '';
        in
        {
          default = pkgs.mkShell {
            packages = [
              rust
              msrv-cargo
              # CI pins cargo-hack 0.6.45 and cargo-semver-checks 0.51.0; flake.lock holds
              # a nixpkgs that ships both, so check them after `nix flake update`.
              pkgs.cargo-hack
              pkgs.cargo-semver-checks
              pkgs.cargo-nextest
              pkgs.uv
              pkgs.python3
            ];
          };
        }
      );
    };
}
