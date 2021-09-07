let
  sources = import ./nix/sources.nix;
  pkgs = import sources.nixpkgs { overlays = [ (import sources.nixpkgs-mozilla) ]; };
  rustNightly = pkgs.latest.rustChannels.nightly.rust.override {
    extensions = [ "rustfmt-preview" "rls-preview" "rust-src" "rust-analyzer-preview" ];
  };
  # the rls is missing on the nightly build of 2020-09-29, so pinning a date for the moment
  rustNightlyTemp = (pkgs.rustChannelOf{
    date = "2021-08-08";
    channel = "nightly";
  }).rust.override {
    extensions = [ "rustfmt-preview" "rls-preview" "rust-src" "rust-analyzer-preview" ];
  };
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustNightlyTemp
    cargo-watch
    postgresql
    pgcli
    openssl.dev
    pkg-config
    diesel-cli
    python3
    python3Packages.flask
    (
      pkgs.writeShellScriptBin "start-dockers" ''
        trap "systemd-run --user --no-block docker stop zauth-db" 0
        docker run --name zauth-db -p 5432:5432 --rm -v zauth-db-data:/var/lib/postgresql/data -e POSTGRES_PASSWORD=zauth -e POSTGRES_USER=zauth postgres:13-alpine -c log_statement=all
        ''
    )
  ];
}
