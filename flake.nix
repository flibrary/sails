{
  description = "FLibrary sails project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.inputs.flake-utils.follows = "utils";
    cargo2nix.url = "github:cargo2nix/cargo2nix/master";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, rust-overlay, utils, cargo2nix, ... }:
    let
      # We customizely define the default system because ghc is broken on aarch64-darwin
      defaultSystems = [
        "aarch64-linux"
        # "aarch64-darwin"
        "i686-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      pkgs = system:
        import nixpkgs {
          system = "${system}";
          overlays = [
            rust-overlay.overlay
            (import "${cargo2nix}/overlay")
            (final: prev: {
              cargo2nix = cargo2nix."${system}".packages.cargo2nix;
            })
          ];
        };
      rustPkgs = system:
        with (pkgs system);
        (rustBuilder.makePackageSet' {
          # appended to "stable"
          rustChannel = "latest";
          packageFun = import ./Cargo.nix;
          # packageOverrides = pkgs: pkgs.rustBuilder.overrides.all; # Implied, if not specified
          packageOverrides = pkgs:
            pkgs.rustBuilder.overrides.all ++ ((import ./overrides-list.nix) pkgs);
        });
    in (utils.lib.eachSystem (defaultSystems) (system: rec {
      # `nix build`
      packages = {
        # We have to do it like `nix develop .#commit` because libraries don't play well with `makeBinPath` or `makeLibraryPath`.
        commit = (import ./commit.nix {
          lib = utils.lib;
          pkgs = (pkgs system);
        });
        sails-bin = ((rustPkgs system).workspace.sails-bin { }).bin;
      };

      defaultPackage = packages.sails-bin;

      # We don't check packages.commit because techinically it is not a pacakge
      checks = builtins.removeAttrs packages [ "commit" ];

      apps = { sails-bin = utils.lib.mkApp { drv = packages.sails-bin; }; };

      defaultApp = apps.sails-bin;

      # `nix develop`
      devShell = with (pkgs system);
        mkShell {
          nativeBuildInputs = [
            # write rustfmt first to ensure we are using nightly rustfmt
            rust-bin.nightly."2021-01-01".rustfmt
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" ];
              targets = [ "x86_64-unknown-linux-musl" ];
            })
            rust-analyzer

            cargo2nix

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

      overlay = final: prev: { sails = (rustPkgs "${prev.pkgs.system}"); };
    };
}
