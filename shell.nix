{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  buildInputs = [
    pkgs.openssl
    pkgs.pkg-config
    pkgs.rust-analyzer
    pkgs.rustup
  ];
}
