{
  description = "FLibrary sails project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "utils";
      };
    };

    cargo2nix = {
      url = "github:flibrary/cargo2nix/master";
      inputs = {
        rust-overlay.follows = "rust-overlay";
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "utils";
      };
    };
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, rust-overlay, utils, cargo2nix, ... }:
    let
      # we are not allowed to use IFD because of the presence of foreign platforms. However, IFD is used in cargo2nix since we use git dependencies.
      # therefore, we temporarily disable the build/eval on aarch64-linux as practically we don't use that platform.
      # info: https://github.com/NixOS/nix/issues/4265
      attrs = (utils.lib.eachSystem ([
        "x86_64-linux"
        # "aarch64-linux"
      ]) (let
        pkgs = system:
          import nixpkgs {
            system = "${system}";
            overlays = [ rust-overlay.overlay cargo2nix.overlay."${system}" ];
          };
        rustPkgs = system:
          with (pkgs system);
          (rustBuilder.makePackageSet' {
            # appended to "stable"
            rustChannel = "latest";
            packageFun = import ./Cargo.nix;
            # packageOverrides = pkgs: pkgs.rustBuilder.overrides.all; # Implied, if not specified
            packageOverrides = pkgs:
              pkgs.rustBuilder.overrides.all
              ++ ((import ./overrides-list.nix) pkgs);
          });
      in system:
      let
        workspaceShell = hook:
          ((rustPkgs system).workspaceShell {
            nativeBuildInputs = with (pkgs system); [
              cargo2nix.packages."${system}".cargo2nix
              rust-bin.nightly."2022-02-15".rustfmt
              # cargo2nix uses the minimal profile which doesn't provide clippy
              rust-bin.stable.latest.clippy
              rust-analyzer
              diesel-cli
              # required by gettext-macro to create PO files
              gettext
            ];
            shellHook = if hook == null then "" else hook;
          });
      in rec {
        # `nix build`
        packages = {
          # We have to do it like `nix develop .#commit` because libraries don't play well with `makeBinPath` or `makeLibraryPath`.
          commit = (workspaceShell (builtins.readFile ./commit.sh));
          sails-bin = ((rustPkgs system).workspace.sails-bin { }).bin;
        };

        defaultPackage = packages.sails-bin;

        apps = {
          sails-bin = utils.lib.mkApp { drv = packages.sails-bin; };
          # sync the migrations
          sync-migrations = utils.lib.mkApp {
            drv = with (pkgs system);
              (writeShellApplication {
                name = "sync-migrations";
                runtimeInputs = [ rsync ];
                text = (builtins.readFile ./sync-migrations.sh);
              });
          };
        };

        defaultApp = apps.sails-bin;

        # `nix develop`
        devShell = workspaceShell null;

        # We don't check packages.commit because techinically it is not a pacakge
        checks = builtins.removeAttrs packages [ "commit" ];
      }));
    in attrs // {
      nixosModule = (import ./module.nix);

      overlay = final: prev: {
        sails-bin = attrs.packages."${prev.pkgs.system}".sails-bin;
      };
    };
}
