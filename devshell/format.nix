{ pkgs, lib }:

pkgs.runCommand "check-format"
  {
    buildInputs = with pkgs; [
      fd

      shellcheck

      nixfmt
      prettier
      shfmt
      taplo
      treefmt
    ];
  }
  ''
    set -e

    src=/tmp/axon-format
    cp -r ${lib.cleanSource ./..} "$src"
    chmod -R u+w "$src"
    cd "$src"

    treefmt \
      --allow-missing-formatter \
      --fail-on-change \
      --no-cache \
      --formatters prettier \
      --formatters nix \
      --formatters shell \
      --formatters hcl \
      --formatters toml

    touch $out
  ''
