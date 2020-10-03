let
  sources = import ./nix/sources.nix;
  pkgs = import sources.nixpkgs { overlays = [ (import sources.nixpkgs-mozilla) ]; };
  rustNightly = pkgs.latest.rustChannels.nightly.rust.override {
    extensions = [ "rustfmt-preview" "rls-preview" "rust-src" "rust-analyzer-preview" ];
  };
  # the rls is missing on the nightly build of 2020-09-29, so pinning a date for the moment
  rustNightlyTemp = (pkgs.rustChannelOf{
    date = "2020-09-28";
    channel = "nightly";
  }).rust.override {
    extensions = [ "rustfmt-preview" "rls-preview" "rust-src" "rust-analyzer-preview" ];
  };
in
pkgs.mkShell {
  buildInputs = [
    rustNightlyTemp
    pkgs.postgresql
    pkgs.pgcli
  ];
}
