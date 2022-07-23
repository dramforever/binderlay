{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.rust-overlay = {
    url = "github:oxalica/rust-overlay";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      eachSystem = nixpkgs.lib.genAttrs [ "x86_64-linux" ];
    in {
      devShell = eachSystem (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          env = { mkShell, rust, targetPlatform, rust-bin }:
            mkShell {
              nativeBuildInputs = [
                (rust-bin.stable.latest.default.override {
                  extensions = [ "rust-src" ];
                  targets = [ (rust.toRustTarget targetPlatform) ];
                })
              ];
            };
        in pkgs.callPackage env {});

      overlays.binderlay = final: prev: {
        binderlay =
          let
            expr = { rustPlatform }:
              rustPlatform.buildRustPackage {
                pname = "binderlay";
                version = "0.1.0";
                src = ./.;
                cargoLock.lockFile = ./Cargo.lock;
              };
          in final.callPackage expr {};
      };

      overlays.default = self.overlays.binderlay;

      packages = eachSystem (system: {
        binderlay = (import nixpkgs {
          inherit system;
          overlays = [ self.overlays.default ];
        }).binderlay;

        default = self.packages.${system}.binderlay;
      });
    };
}
