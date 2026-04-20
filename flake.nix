{
  description = "hortpro-checker - daycare attendance monitor via HortPro Elternportal";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        hortpro-checker = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;

          postInstall = ''
            wrapProgram "$out/bin/hortpro-checker" \
              --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.libnotify ]}
          '';

          nativeBuildInputs = [ pkgs.makeWrapper ];

          meta = with pkgs.lib; {
            description = "Daycare attendance monitor via HortPro Elternportal";
            homepage = "https://github.com/soenkeliebau/hortpro-checker";
            license = licenses.asl20;
            mainProgram = "hortpro-checker";
          };
        });
      in
      {
        checks = {
          inherit hortpro-checker;

          hortpro-checker-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });

          hortpro-checker-fmt = craneLib.cargoFmt {
            src = craneLib.cleanCargoSource ./.;
          };

          hortpro-checker-test = craneLib.cargoTest (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        packages = {
          default = hortpro-checker;
          hortpro-checker = hortpro-checker;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = hortpro-checker;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs; [
            cargo-watch
            rust-analyzer
            libnotify
          ];
        };
      }
    );
}
