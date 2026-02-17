{
  description = "Axon - ";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
      crane,
    }:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
    in
    (flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            self.overlays.default
            fenix.overlays.default
          ];
        };

        rustToolchain =
          with fenix.packages.${system};
          combine [
            stable.rustc
            stable.cargo
            stable.clippy
            stable.rust-src
            stable.rust-std

            default.rustfmt
          ];

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        cargoArgs = [
          "--workspace"
          "--bins"
          "--examples"
          "--tests"
          "--benches"
          "--all-targets"
        ];

        unitTestArgs = [
          "--workspace"
        ];

      in
      {
        formatter = pkgs.treefmt;

        devShells.default = pkgs.callPackage ./devshell {
          inherit rustToolchain cargoArgs unitTestArgs;
        };

        packages = rec {
          default = axon;
          axon = pkgs.callPackage ./devshell/package.nix {
            inherit (cargoToml.workspace.metadata.crane) name;
            inherit (cargoToml.workspace.package) version;
            inherit rustPlatform;
          };
        };
      }
    ))
    // {
      overlays.default = final: prev: { };
    };
}
