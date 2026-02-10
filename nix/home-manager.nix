{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.wayland.compositor.cosmic-comp;
  settingsFormat = pkgs.formats.toml { };
in
{
  options.wayland.compositor.cosmic-comp = {
    enable = mkEnableOption "COSMIC compositor (with RemoteDesktop EIS support) via Home Manager";

    package = mkPackageOption pkgs "cosmic-comp" {
      default = [ "cosmic-comp" ];
      example = literalExpression ''
        pkgs.cosmic-comp
      '';
    };

    xkb = {
      layout = mkOption {
        type = types.str;
        default = "us";
        description = "XKB keyboard layout for the compositor.";
      };
      variant = mkOption {
        type = types.str;
        default = "";
        description = "XKB layout variant.";
      };
      options = mkOption {
        type = types.str;
        default = "";
        description = "XKB options (e.g., 'ctrl:nocaps').";
      };
      model = mkOption {
        type = types.str;
        default = "";
        description = "XKB model.";
      };
    };

    extraConfig = mkOption {
      type = types.attrsOf types.anything;
      default = { };
      description = ''
        Additional cosmic-comp configuration written via cosmic-config.
        These are merged into the compositor's TOML configuration.
      '';
    };
  };

  config = mkIf cfg.enable {
    home.packages = [ cfg.package ];

    # Write XKB configuration to cosmic-config directory
    xdg.configFile = let
      xkbConfig = {
        rules = "";
        model = cfg.xkb.model;
        layout = cfg.xkb.layout;
        variant = cfg.xkb.variant;
        options = cfg.xkb.options;
      };
    in mkIf (cfg.xkb.layout != "us" || cfg.xkb.variant != "" || cfg.xkb.options != "") {
      "cosmic-comp/v1/xkb-config".source =
        settingsFormat.generate "cosmic-comp-xkb" xkbConfig;
    };
  };
}
