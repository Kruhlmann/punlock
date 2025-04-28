{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  buildInputs = [
    pkgs.cargo-llvm-cov
    pkgs.openssl
    pkgs.pkg-config
    pkgs.rust-analyzer
    pkgs.rustup
  ];
}
