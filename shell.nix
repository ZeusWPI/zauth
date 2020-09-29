let
  sources = import ./nix/sources.nix;
  pkgs = import sources.nixpkgs { overlays = [ (import sources.nixpkgs-mozilla) ]; };
  rustnightly = pkgs.latest.rustChannels.stable.rust.override {
    extensions = [ "rustfmt-preview" "rls-preview" ];
  };
in
pkgs.mkShell {
  buildInputs = [
    rustnightly
    pkgs.postgresql
    pkgs.pgcli
  ];
}
