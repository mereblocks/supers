{
  description = "Programmable supervisor for long-running programs.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    shell-utils.url = "github:waltermoreira/shell-utils";
    taskdep.url = "github:waltermoreira/taskdep";
  };

  outputs = { self, nixpkgs, flake-utils, crane, shell-utils, taskdep }:

    with flake-utils.lib; eachSystem [
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
          packages.default = packages.supers;
          devShells.default = shell {
            packages = with pkgs; [
              packages.supers
              cargo
              rustc
              jq
              go-task
              taskdep-bin
            ];
          };
        });
}
