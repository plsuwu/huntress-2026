{
  description = "Huntress 2026 web-only solver pipeline";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = import nixpkgs { inherit system; };
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            python3
            curl
            jq
            ripgrep
            file
            p7zip
            unzip
            uv
            bun
            sqlmap
          ];
        };
      });
}
