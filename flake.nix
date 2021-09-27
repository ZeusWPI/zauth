{
  description = "Simple OAuth2 client for mail clients";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = {self,  nixpkgs, flake-utils, rust-overlay, ... }:
  flake-utils.lib.eachDefaultSystem (system:
  let
    overlays = [ (import rust-overlay) ];
    pkgs = import nixpkgs {
      inherit system overlays;
    };
  in
  with pkgs;
  {
    devShell = mkShell {
      buildInputs = [
        rust-bin.nightly.latest.default
        openssl.dev
        pkg-config
        docker-compose
        cargo-watch
        postgresql
        pgcli
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
    };
  });
}
