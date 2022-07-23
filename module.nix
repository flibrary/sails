{ config, pkgs, lib, ... }:

with lib;

let
  cfg = config.sails;
  toTOML = (import ./to-toml.nix { inherit lib; });
  confFile = if cfg.configFile != null then
    cfg.configFile
  else
    pkgs.writeText "sails-config.toml" (toTOML cfg.config);
in {
  options.sails = {
    enable = mkOption {
      type = types.bool;
      default = false;
    };

    config = mkOption {
      type = types.nullOr types.unspecified;
      default = null;
      description = "Rocket.toml compatiable config";
    };

    configFile = mkOption {
      type = types.nullOr types.path;
      default = null;
      description = "Path to the Rocket.toml compatiable config file";
    };

    dataDir = mkOption {
      type = types.path;
      default = "/var/lib/sails";
      description = "The data dir that the service has access with";
    };

    requires = mkOption {
      type = types.listOf types.str;
      description =
        "The services to be specified in `Requires` of the systemd config";
    };

    after = mkOption {
      type = types.listOf types.str;
      description =
        "The services to be specified in `After` of the systemd config";
    };

    wants = mkOption {
      type = types.listOf types.str;
      description =
        "The services to be specified in `wants` of the systemd config";
    };

    package = mkOption {
      type = types.package;
      description = "Package of the sails-bin";
    };
  };
  config = mkIf cfg.enable {
    assertions = [{
      assertion = (cfg.configFile == null) != (cfg.config == null);
      message =
        "Either but not both `configFile` and `config` should be specified for sails.";
    }];

    users.users.sails = {
      description = "Sails server daemon user";
      group = "sails";
      home = cfg.dataDir;
      createHome = true;
      # seems like this UID has not been used yet https://github.com/NixOS/nixpkgs/blob/master/nixos/modules/misc/ids.nix
      uid = 400;
    };
    users.groups.sails = { };

    systemd.services.sails = {
      description = "Sails Server Service";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ] ++ cfg.after;
      requires = cfg.requires;
      wants = cfg.wants;

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/sails-bin --config ${confFile}";
        User = "sails";
        AmbientCapabilities = "CAP_NET_BIND_SERVICE";
        Restart = "on-failure";

        # WorkingDirectory = cfg.dataDir;
        PrivateTmp = true;
        # Users Database is not available for within the unit, only root and minecraft is available, everybody else is nobody
        PrivateUsers = true;
        # Read only mapping of /usr /boot and /etc
        ProtectSystem = "full";
        # /home, /root and /run/user seem to be empty from within the unit.
        ProtectHome = true;
        # /proc/sys, /sys, /proc/sysrq-trigger, /proc/latency_stats, /proc/acpi, /proc/timer_stats, /proc/fs and /proc/irq will be read-only within the unit.
        ProtectKernelTunables = true;
        # Block module system calls, also /usr/lib/modules.
        ProtectKernelModules = true;
        ProtectControlGroups = true;
      };
    };
  };
}
