pkgs: [
  (pkgs.rustBuilder.rustLib.makeOverride {
    name = "askama_escape";
    overrideAttrs = drv: {
      postPatch = ''
        substituteInPlace ./askama_escape/Cargo.toml --replace "workspace = \"..\"" "" '';
    };
  })
  (pkgs.rustBuilder.rustLib.makeOverride {
    name = "askama_shared";
    overrideAttrs = drv: {
      postPatch = ''
        substituteInPlace ./askama_shared/Cargo.toml --replace "workspace = \"..\"" "" '';
    };
  })
  (pkgs.rustBuilder.rustLib.makeOverride {
    name = "askama_derive";
    overrideAttrs = drv: {
      postPatch = ''
        substituteInPlace ./askama_derive/Cargo.toml --replace "workspace = \"..\"" "" '';
    };
  })
  (pkgs.rustBuilder.rustLib.makeOverride {
    name = "askama_rocket";
    overrideAttrs = drv: {
      postPatch = ''
        substituteInPlace ./askama_rocket/Cargo.toml --replace "workspace = \"..\"" "" '';
    };
  })
  (pkgs.rustBuilder.rustLib.makeOverride {
    name = "askama";
    overrideAttrs = drv: {
      postPatch = ''
        substituteInPlace ./askama/Cargo.toml --replace "workspace = \"..\"" "" '';
    };
  })
  (pkgs.rustBuilder.rustLib.makeOverride {
    name = "migrations_macros";
    overrideAttrs = drv: {
      propagatedNativeBuildInputs = drv.propagatedNativeBuildInputs or [ ]
        ++ [ pkgs.sqlite ];
    };
  })

  (pkgs.rustBuilder.rustLib.makeOverride {
    name = "sails-bin";
    overrideAttrs = drv: {
      propagatedNativeBuildInputs = drv.propagatedNativeBuildInputs or [ ]
        ++ [ pkgs.sqlite pkgs.gettext ];
    };
  })
]
