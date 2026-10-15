{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        toolchainFor =
          p:
          p.rust-bin.selectLatestNightlyWith (
            tc:
            tc.default.override {
              extensions = [ "rust-src" ];
              targets = [ "x86_64-unknown-linux-gnu" ];
            }
          );
        craneLib = (crane.mkLib pkgs).overrideToolchain toolchainFor;

        # Static musl build of ctf-run (with https) that runs unchanged on the jumpbox (Ubuntu),
        # so release binaries are built here instead of burning the t3.small's CPU credits.
        # `nix build .#ctf-run` -> result/bin/ctf-run
        muslTarget = "x86_64-unknown-linux-musl";
        muslCraneLib = (crane.mkLib pkgs).overrideToolchain (
          p: p.rust-bin.stable.latest.default.override { targets = [ muslTarget ]; }
        );
        muslCc = pkgs.pkgsStatic.stdenv.cc;
        muslCcBin = "${muslCc}/bin/${muslCc.targetPrefix}cc";
        ctf-run = muslCraneLib.buildPackage {
          pname = "ctf-run";
          src = muslCraneLib.cleanCargoSource ./ctf-run;
          strictDeps = true;
          doCheck = false;
          cargoExtraArgs = "--locked --features tls";
          CARGO_BUILD_TARGET = muslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
          # ring compiles C/assembly: use the musl toolchain for the target, the normal one for build scripts.
          CC_x86_64_unknown_linux_musl = muslCcBin;
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = muslCcBin;
          HOST_CC = "${pkgs.stdenv.cc}/bin/cc";
        };
      in
      {
        packages = {
          inherit ctf-run;
          default = ctf-run;
        };

        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            pkg-config
            bun
            uv
            xxd
            ilspycmd
            capstone
          ];
        };
      }
    );
}
