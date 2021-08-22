{
  description = "FLibrary sails project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    utils.url = "github:numtide/flake-utils";
    # This is required for recursive dependency
    naersk.url = "github:nmattia/naersk/pull/167/head";
  };

  outputs = { nixpkgs, rust-overlay, utils, naersk, ... }:
    let
      pkgWith = system:
        naersk.lib."${system}".buildPackage {
          name = "sails-bin";
          version = "git";
          root = ./.;
          # Otherwise Nix tries to use `/bin/sails-bin-git`
          passthru.exePath = "/bin/sails-bin";
          nativeBuildInputs = with import nixpkgs { system = "${system}"; }; [
            # used by email
            openssl
            pkgconfig
            # Used by diesel
            sqlite
          ];
        };
    in (utils.lib.eachSystem (utils.lib.defaultSystems) (system: rec {
      # `nix build`
      packages = {
        # We have to do it like `nix develop .#commit` because libraries don't play well with `makeBinPath` or `makeLibraryPath`.
        commit = (import ./commit.nix {
          lib = utils.lib;
          pkgs = import nixpkgs {
            system = "${system}";
            overlays = [ rust-overlay.overlay ];
          };
        });
        sails-bin = (pkgWith "${system}");
      };

      defaultPackage = packages.sails-bin;

      # We don't check packages.commit because techinically it is not a pacakge
      checks = builtins.removeAttrs packages [ "commit" ];

      apps = { sails-bin = utils.lib.mkApp { drv = packages.sails-bin; }; };

      defaultApp = apps.sails-bin;

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

            # used by email
            openssl
            pkgconfig
            # Used by diesel
            sqlite

            diesel-cli

            binutils-unwrapped
          ];
        };
    })) // {
      nixosModule = (import ./module.nix);

      overlay = final: prev: { sails = (pkgWith "${prev.pkgs.system}"); };
    };
}
