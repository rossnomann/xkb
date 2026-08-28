{
  description = "XKB Configuration";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };
  outputs =
    inputs:
    let
      system = "x86_64-linux";
      pkgs = inputs.nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = [
          pkgs.python314
          pkgs.just
          pkgs.libxkbcommon
        ];
      };
      overlays.${system}.default = final: _: {
        custom-xkb-symbols = final.callPackage ./package.nix { };
      };
      packages.${system}.default = pkgs.callPackage ./package.nix { };
    };
}
