{
  description = "FLibrary sails project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nmattia/naersk";
  };

  outputs = { nixpkgs, rust-overlay, utils, naersk, ... }:
    utils.lib.eachSystem (utils.lib.defaultSystems) (system: rec {
      # `nix build`
      packages.sails-bin = (naersk.lib."${system}".buildPackage {
        name = "sails-bin";
        version = "git";
        root = ./.;
        passthru.exePath = "/bin/sails-bin";
        nativeBuildInputs = with import nixpkgs { system = "${system}"; }; [
          # used by check_email
          openssl
          pkg-config
          # Used by diesel
          sqlite
        ];
      });

      defaultPackage = packages.sails-bin;

      checks = packages;

      apps = {
        commit = (import ./commit.nix {
          lib = utils.lib;
          pkgs = import nixpkgs {
            system = "${system}";
            overlays = [ rust-overlay.overlay ];
          };
        });
      };

      # `nix develop`
      devShell = with import nixpkgs {
        system = "${system}";
        overlays = [ rust-overlay.overlay ];
      };
        mkShell {
          nativeBuildInputs = [
            # write rustfmt first to ensure we are using nightly rustfmt
            rust-bin.nightly."2021-01-01".rustfmt
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" ];
              targets = [ "x86_64-unknown-linux-musl" ];
            })
            rust-analyzer

            # used by check_email
            openssl
            pkg-config
            # Used by diesel
            sqlite

            diesel-cli

            binutils-unwrapped
          ];
        };
    });
}
