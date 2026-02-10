{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.cosmic-comp;
in
{
  options.services.cosmic-comp = {
    enable = mkEnableOption "COSMIC compositor (with RemoteDesktop EIS support)";

    package = mkPackageOption pkgs "cosmic-comp" {
      default = [ "cosmic-comp" ];
      example = literalExpression ''
        pkgs.cosmic-comp
      '';
    };

    eis = {
      enable = mkOption {
        type = types.bool;
        default = true;
        description = ''
          Whether to enable the EIS (Emulated Input Server) D-Bus interface.

          This allows the xdg-desktop-portal-cosmic RemoteDesktop portal to
          inject keyboard and mouse input into the compositor for remote
          desktop sessions.
        '';
      };
    };

    settings = mkOption {
      type = types.submodule {
        freeformType = (pkgs.formats.toml { }).type;
        options = {
          xkb-config = mkOption {
            type = types.submodule {
              freeformType = (pkgs.formats.toml { }).type;
              options = {
                rules = mkOption {
                  type = types.str;
                  default = "";
                  description = "XKB rules.";
                };
                model = mkOption {
                  type = types.str;
                  default = "";
                  description = "XKB model.";
                };
                layout = mkOption {
                  type = types.str;
                  default = "us";
                  description = "XKB keyboard layout.";
                };
                variant = mkOption {
                  type = types.str;
                  default = "";
                  description = "XKB layout variant.";
                };
                options = mkOption {
                  type = types.str;
                  default = "";
                  description = "XKB options.";
                };
              };
            };
            default = { };
            description = "XKB keyboard configuration.";
          };
        };
      };
      default = { };
      description = ''
        Configuration for the COSMIC compositor.

        Settings are passed via cosmic-config.
      '';
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    # The compositor is launched by the display manager (greetd/cosmic-greeter)
    # as the Wayland session compositor. It is NOT a systemd service.

    # Required system services
    hardware.graphics.enable = mkDefault true;
    services.seatd.enable = mkDefault true;

    # Ensure D-Bus is available for EIS interface
    services.dbus.packages = mkIf cfg.eis.enable [ cfg.package ];

    # udev rules for input devices
    services.udev.packages = [ cfg.package ];

    # Security: allow the compositor to access DRM, input devices
    security.polkit.enable = mkDefault true;

    # The compositor needs access to /dev/dri/* and /dev/input/*
    # This is handled by seatd/logind integration.
  };
}
