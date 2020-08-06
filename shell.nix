let
  sources = import ./nix/sources.nix;
  rust = import ./nix/rust.nix { inherit sources; };
  pkgs = import sources.nixpkgs { };
in
pkgs.mkShell {
  buildInputs = [
    rust
    pkgs.sqlite
    pkgs.mariadb
  ];
  LD_PRELOAD =
    "/nix/store/2d83pqzzi4qxsg1nfppp1qrkr6hn323g-mariadb-server-10.4.13/lib/libmariadb.so";
}
