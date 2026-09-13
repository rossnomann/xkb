{
  description = "XKB Configuration";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    inputs:
    let
      system = "x86_64-linux";
      overlays = [ inputs.rust-overlay.overlays.default ];
      pkgs = import inputs.nixpkgs { inherit system overlays; };
      rust-dev = (
        pkgs.rust-bin.selectLatestNightlyWith (
          toolchain:
          toolchain.minimal.override {
            extensions = [
              "rust-analyzer"
              "rust-src"
              "rustfmt"
            ];
          }
        )
      );
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        RUST_SRC_PATH = "${rust-dev}/lib/rustlib/src/rust/library";
        buildInputs = [
          pkgs.just
          pkgs.libxkbcommon
          pkgs.python314
          (pkgs.lib.hiPrio (
            pkgs.rust-bin.stable.latest.minimal.override {
              extensions = [
                "rust-docs"
                "clippy"
                "llvm-tools"
              ];
            }
          ))
          rust-dev
        ];
        shellHook = ''
          export CARGO_HOME="$PWD/.cargo"
          export PATH="$CARGO_HOME/bin:$PATH"
          mkdir -p .cargo
          echo '*' > .cargo/.gitignore
        '';
      };
      overlays.${system}.default = final: _: {
        custom-xkb-symbols = final.callPackage ./package.nix { };
      };
      packages.${system}.default = pkgs.callPackage ./package.nix { };
    };
}
