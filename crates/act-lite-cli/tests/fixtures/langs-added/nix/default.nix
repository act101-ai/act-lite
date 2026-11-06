{ pkgs ? import <nixpkgs> {}, lib, ... }:
let
  version = "1.0";
  mkTool = name: pkgs.writeShellScriptBin name "echo ${name}";
  common = import ./common.nix { inherit lib; };
in
{
  inherit version;
  packages.default = mkTool "shop";
  services.web = { enable = true; port = 8080; };
  check = if version == "1.0" then true else false;
}
