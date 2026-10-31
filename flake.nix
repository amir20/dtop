# Nix flake for dtop
#
# Usage:
#   nix run github:amir20/dtop          # Run using pre-built binary (fast)
#   nix run github:amir20/dtop#source   # Run building from source
#
# UPDATING (when releasing a new version):
#   ./scripts/update-nix-hashes.sh <VERSION>
{
  description = "Terminal dashboard for Docker monitoring across multiple hosts";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        version = "0.10.5";

        # Binary release configuration
        platformMap = {
          "x86_64-linux" = "x86_64-unknown-linux-gnu";
          "aarch64-linux" = "aarch64-unknown-linux-gnu";
          "x86_64-darwin" = "x86_64-apple-darwin";
          "aarch64-darwin" = "aarch64-apple-darwin";
        };

        hashes = {
          "x86_64-linux" = "sha256-Qh3Mfy25z09yJds7KLmaJauxr4lYhsjBPON5SjneAaM=";
          "aarch64-linux" = "sha256-uGbYrHXfw/3SQhc0zdnbYFSmBiwbE4m0RS5vd/OWwnU=";
          "x86_64-darwin" = "sha256-VsCGMfEGHjr3kWU/+y4yrnyvxBpJfNHe2CQay//5hpY=";
          "aarch64-darwin" = "sha256-U1qWJ6moBZEbj3WYYxb61ebs3RIvxVhHPTW6FphQx/M=";
        };

        platform = platformMap.${system} or (throw "Unsupported system: ${system}");

        meta = {
          description = "Terminal dashboard for Docker monitoring across multiple hosts with Dozzle integration";
          homepage = "https://github.com/amir20/dtop";
          changelog = "https://github.com/amir20/dtop/blob/main/CHANGELOG.md";
          license = pkgs.lib.licenses.mit;
          mainProgram = "dtop";
          platforms = pkgs.lib.platforms.unix;
        };
      in
      {
        packages = {
          # Default: pre-built binary (fast install)
          default = pkgs.stdenv.mkDerivation {
            pname = "dtop";
            inherit version meta;

            src = pkgs.fetchurl {
              url = "https://github.com/amir20/dtop/releases/download/v${version}/dtop-${platform}.tar.gz";
              hash = hashes.${system};
            };

            sourceRoot = "dtop-${platform}";

            nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              pkgs.autoPatchelfHook
            ];

            buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              pkgs.openssl
              pkgs.stdenv.cc.cc.lib
            ];

            dontConfigure = true;
            dontBuild = true;

            installPhase = ''
              runHook preInstall
              install -Dm755 dtop $out/bin/dtop
              runHook postInstall
            '';
          };

          # Build from source
          source = pkgs.rustPlatform.buildRustPackage {
            pname = "dtop";
            inherit version meta;

            src = pkgs.lib.cleanSource ./.;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [ pkgs.pkg-config ];

            # On Darwin, Security and SystemConfiguration come from the default SDK.
            buildInputs = [ pkgs.openssl ];

            # Disable auto update
            buildNoDefaultFeatures = true;
          };
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.source ];
          packages = with pkgs; [
            cargo
            rustc
            rust-analyzer
            clippy
            rustfmt
          ];
        };

        formatter = pkgs.nixfmt-tree;
      }
    );
}
