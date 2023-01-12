{
  description = "Programmable supervisor for long-running programs.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    shell-utils.url = "github:waltermoreira/shell-utils";
    taskdep.url = "github:waltermoreira/taskdep";
    fblog = {
      url = "github:brocode/fblog";
      flake = false;
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils
    , crane
    , shell-utils
    , taskdep
    , fblog
    , rust-overlay
    }:

    let
      dynamic = with flake-utils.lib; eachSystem [
        system.x86_64-linux
        system.x86_64-darwin
      ]
        (system:
          let
            pkgs = nixpkgs.legacyPackages.${system};
            craneLib = crane.lib.${system};
            shell = shell-utils.myShell.${system};
            taskdep-bin = taskdep.packages.${system}.taskdep;
          in
          rec {
            packages.supers =
              craneLib.buildPackage {
                src = craneLib.cleanCargoSource ./.;
                buildInputs = with pkgs; [
                  libiconv
                ];
              };
            packages.fblog =
              craneLib.buildPackage {
                src = fblog;
                buildInputs = with pkgs; [
                  libiconv
                ];
              };
            packages.default = packages.supers;
            devShells.default = shell {
              packages = with pkgs; [
                packages.supers
                packages.fblog
                cargo
                rustc
                jq
                go-task
                taskdep-bin
              ];
            };
          });
      static = flake-utils.lib.eachSystem [ "x86_64-linux" ]
        (system:
          let
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ (import rust-overlay) ];
            };
            rustToolchain = pkgs.rust-bin.stable.latest.default.override {
              targets = [ "x86_64-unknown-linux-musl" ];
            };
            craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
          in
          {
            packages.static-supers = craneLib.buildPackage {
              src = craneLib.cleanCargoSource ./.;
              CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
              CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
              buildInputs = [ pkgs.libiconv ];
            };
          });
    in
    nixpkgs.lib.recursiveUpdate dynamic static;
}
