{ pkgs, lib }:
with pkgs;

pkgs.mkShell {
  # this will make all the build inputs from hello and gnutar
  # available to the shell environment
  nativeBuildInputs = [
    git
    coreutils
    findutils
    nixfmt
    gcc

    # write rustfmt first to ensure we are using nightly rustfmt
    rust-bin.nightly."2021-01-01".rustfmt
    rust-bin.stable.latest.default
    binutils-unwrapped
    sqlite
    openssl
    pkgconfig
  ];

  shellHook = ''
    set -e

    find . -type f -name '*.nix' -exec nixfmt {} +
    cargo update
    cargo fmt -- --check
    cargo build --all-features
    cargo test
    cargo clippy
    cargo bench --no-run

    echo -n "Adding to git..."
    git add --all
    echo "Done."

    git status
    read -n 1 -s -r -p "Press any key to continue"

    echo "Commiting..."
    echo "Enter commit message: "
    read -r commitMessage
    git commit -m "$commitMessage"
    echo "Done."

    echo -n "Pushing..."
    git push
    echo "Done."

    exit
  '';
}
