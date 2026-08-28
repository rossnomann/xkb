{ stdenvNoCC }:
stdenvNoCC.mkDerivation {
  pname = "custom-xkb-symbols";
  src = ./xkb/symbols;
  version = "0.0.0";
  installPhase = ''
    runHook preInstall
    mkdir -p $out/share/X11/xkb/symbols
    cp -a custom_lat $out/share/X11/xkb/symbols/
    cp -a custom_cyr $out/share/X11/xkb/symbols/
    runHook postInstall
  '';
}
