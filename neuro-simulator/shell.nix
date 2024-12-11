{ pkgs ? import <nixpkgs> { }, lib ? pkgs.lib }:

pkgs.mkShell rec {
  name = "rust-env";

  nativeBuildInputs = with pkgs; [
    pkg-config
    rustc
    cargo
  ];
  buildInputs = with pkgs; [
    # alsa-lib
    # systemdLibs
    vulkan-loader
    # libGL
    # openssl
    # xorg.libX11
    # xorg.libXcursor
    # xorg.libXrandr
    # xorg.libXi
    wayland
    libxkbcommon
  ];

  LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
}
