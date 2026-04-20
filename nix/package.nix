# Standalone derivation for use in NixOS/home-manager configs.
#
# Usage with fetchFromGitHub:
#
#   let
#     hortpro-checker = pkgs.callPackage (import (pkgs.fetchFromGitHub {
#       owner = "soenkeliebau";
#       repo = "hortpro-checker";
#       rev = "main";
#       hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
#     } + "/nix/package.nix")) {};
#   in
#   {
#     environment.systemPackages = [ hortpro-checker ];
#   }
#
# Or if you already have the flake as an input, prefer using the flake package
# output instead of this file.
{ lib
, rustPlatform
, makeWrapper
, libnotify
}:

rustPlatform.buildRustPackage {
  pname = "hortpro-checker";
  version = "0.1.0";

  src = lib.cleanSource ./..;

  # Replace with the real hash after first build attempt.
  # Run `nix-build` once and Nix will report the correct hash.
  cargoHash = "sha256-4jtfILEpAchXofN78Q9lemKGDb8tr7wC96th42dKy2E=";

  nativeBuildInputs = [
    makeWrapper
  ];

  postInstall = ''
    wrapProgram "$out/bin/hortpro-checker" \
      --prefix PATH : ${lib.makeBinPath [ libnotify ]}
  '';

  meta = with lib; {
    description = "Daycare attendance monitor via HortPro Elternportal";
    homepage = "https://github.com/soenkeliebau/hortpro-checker";
    license = licenses.asl20;
    mainProgram = "hortpro-checker";
    platforms = platforms.linux;
  };
}
