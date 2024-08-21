{
  description = "Rust shells";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "x86_64-darwin"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      perSystem =
        {
          config,
          pkgs,
          system,
          ...
        }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
          };
        in
        {
          devShells.default = pkgs.mkShell {
            buildInputs = with pkgs; [
              alejandra
              just
              rust-bin.stable.latest.default

              # deps for bevy
              alsa-lib.dev
              udev.dev
              openssl
              pkg-config
            ];
          };
        };
    };
}
