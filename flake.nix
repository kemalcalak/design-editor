{
  description = "Design Editor native editor, web host, and web SDK flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    rs-harbor = {
      url = "git+https://codeberg.org/caniko/rs-harbor.git?ref=trunk";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-parts.follows = "flake-parts";
      inputs.crane.follows = "crane";
      inputs.rust-overlay.follows = "rust-overlay";
    };
    py-harbor = {
      url = "git+https://codeberg.org/caniko/py-harbor.git?ref=trunk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    js-harbor = {
      url = "git+https://codeberg.org/caniko/js-harbor.git?ref=trunk";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-parts.follows = "flake-parts";
    };

    nix-appimage.url = "github:ralismark/nix-appimage";

    nix-pklx = {
      url = "git+https://codeberg.org/caniko/nix-pklx.git";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    agent = {
      url = "github:ZSeven-W/agent-rs/5c0f9506e27be5b4f29cd2c1093e2858ad0fa20c";
      flake = false;
    };
    casement = {
      url = "github:ZSeven-W/casement/ffdd672cbda3abe5cd78d056fb75683f2a74f7fe";
      flake = false;
    };
    jian = {
      url = "github:ZSeven-W/jian/2edfc361cff3f6f33174746c048002607d691d1c";
      flake = false;
    };
  };

  outputs = inputs @ {
    self,
    flake-parts,
    nixpkgs,
    crane,
    rust-overlay,
    rs-harbor,
    py-harbor,
    js-harbor,
    nix-appimage,
    nix-pklx,
    agent,
    casement,
    jian,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux"];

      flake = {
        lib.integrationManifest = import ./nix/integration/design-editor.nix;
      };

      perSystem = {system, ...}: let
        lib = pkgs.lib;
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };
        version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
        releaseManifest = builtins.fromJSON (builtins.readFile ./nix/release-manifest.json);
        releaseVersion = releaseManifest.version;

        toolchain = rs-harbor.lib.mkToolchain {
          inherit pkgs;
          toolchainFile = ./rust-toolchain.toml;
          cache.enable = false;
        };
        craneLib = toolchain.craneLib;
        rustNativeInputs = rs-harbor.lib.mkRustNativeBuildInputs {
          inherit pkgs;
          extra = [pkgs.pkg-config pkgs.cmake pkgs.ninja];
        };
        nativeLibraries = with pkgs; [
          fontconfig
          freetype
          libGL
          libdrm
          libglvnd
          libxkbcommon
          mesa
          wayland
          libx11
          libxcursor
          libxext
          libxfixes
          libxi
          libxrandr
          libxrender
          libxcb
          libxcb-util
          libxcb-image
          libxcb-keysyms
          libxcb-render-util
          libxcb-wm
        ];
        runtimeLibraries = nativeLibraries;
        # Release archives are Ubuntu-built ELF binaries.  Patch their
        # interpreter and shared-library references into the Nix store before
        # wrapping them, otherwise they fail before LD_LIBRARY_PATH is read.
        prebuiltDesktopRuntimeLibraries =
          runtimeLibraries
          ++ [
            pkgs.openssl
            pkgs.zlib
            pkgs.stdenv.cc.cc.lib
          ];
        prebuiltCliRuntimeLibraries = [pkgs.stdenv.cc.cc.lib];

        source = pkgs.runCommand "design-editor-source-${version}" {} ''
          mkdir -p $out/vendor/agent $out/vendor/casement $out/vendor/jian
          cp -R --no-preserve=ownership ${./.}/. $out/
          rm -rf $out/vendor/agent $out/vendor/casement $out/vendor/jian
          cp -R --no-preserve=ownership ${agent}/. $out/vendor/agent
          cp -R --no-preserve=ownership ${casement}/. $out/vendor/casement
          cp -R --no-preserve=ownership ${jian}/. $out/vendor/jian
        '';

        skiaBinaries = pkgs.fetchurl {
          url = "https://github.com/rust-skia/skia-binaries/releases/download/0.97.2/skia-binaries-da8fc6731fc439bc3b6a-x86_64-unknown-linux-gnu-gl-jpegd-jpege-pdf-textlayout.tar.gz";
          hash = "sha256-7nf70Bg+hU4pcnZwXk6GhYN8bH0DBEcslxRfzY9/LPw=";
        };
        skiaRepositoryHash = "da8fc6731fc439bc3b6aa90d63506a808f806b26";
        skiaGit = pkgs.writeShellScriptBin "git" ''
          if [ "$1" = rev-parse ] && [ "$2" = --short=20 ] && [ "$3" = HEAD ]; then
            printf '%s\n' ${skiaRepositoryHash}
            exit 0
          fi
          exec ${pkgs.git}/bin/git "$@"
        '';
        nativeEnv = {
          SKIA_BINARIES_URL = "file://${skiaBinaries}";
          FORCE_SKIA_BINARIES_DOWNLOAD = "1";
        };
        commonArgs = {
          inherit version;
          src = ./.;
          cargoLock = ./Cargo.lock;
          strictDeps = true;
          cargoArtifacts = null;
          doCheck = false;
          env = nativeEnv;
          nativeBuildInputs = rustNativeInputs ++ [pkgs.makeWrapper pkgs.python3 pkgs.perl skiaGit];
          buildInputs = nativeLibraries;
          rsHarborCargoTomlContents = builtins.readFile ./Cargo.toml;
          preBuild = ''
            # skia-bindings uses the packaged rust-skia commit to select its
            # prebuilt archive. Recreate that metadata in the Nix source tree
            # because Cargo vendor sources do not retain the upstream Git repo.
            mkdir -p .git/refs/heads
            printf 'ref: refs/heads/main\n' > .git/HEAD
            printf '%s\n' ${skiaRepositoryHash} > .git/refs/heads/main
            mkdir -p vendor
            rm -rf vendor/agent vendor/casement vendor/jian
            cp -R --no-preserve=ownership ${agent} vendor/agent
            cp -R --no-preserve=ownership ${casement} vendor/casement
            cp -R --no-preserve=ownership ${jian} vendor/jian
            export SKIA_BINARIES_URL=${lib.escapeShellArg nativeEnv.SKIA_BINARIES_URL}
            export FORCE_SKIA_BINARIES_DOWNLOAD=1
          '';
        };

        nativePackage = craneLib.buildPackage (commonArgs
          // {
            pname = "design-editor-native";
            buildInputs = commonArgs.buildInputs ++ [pkgs.openssl];
            cargoBuildCommand = "cargo build --release --package de-host-desktop --package de-cli --package de-host-web-server";
            installPhase = ''
              runHook preInstall
              install -Dm755 target/release/design-editor $out/bin/design-editor
              install -Dm755 target/release/de $out/bin/de
              install -Dm755 target/release/de-host-web-server $out/bin/de-host-web-server
              install -Dm644 crates/de-host-desktop/assets/icon.png $out/share/icons/hicolor/1024x1024/apps/design-editor.png
              install -Dm644 ${pkgs.writeText "design-editor.desktop" ''
                [Desktop Entry]
                Name=Design Editor
                Comment=Open-source AI-native vector design tool
                Exec=design-editor %U
                Terminal=false
                Type=Application
                Icon=design-editor
                Categories=Graphics;
                MimeType=application/x-design-editor;
              ''} $out/share/applications/design-editor.desktop
              install -Dm644 ${pkgs.writeText "design-editor.xml" ''
                <?xml version="1.0" encoding="UTF-8"?>
                <mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
                  <mime-type type="application/x-design-editor">
                    <comment>Design Editor Document</comment>
                    <glob pattern="*.de"/>
                    <glob pattern="*.pen"/>
                  </mime-type>
                </mime-info>
              ''} $out/share/mime/packages/design-editor.xml
              install -Dm644 crates/de-host-desktop/assets/icon.png $out/share/pixmaps/design-editor.png
              runHook postInstall
            '';
            postFixup = ''
              for program in design-editor de de-host-web-server; do
                wrapProgram "$out/bin/$program" \
                  --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibraries}
              done
            '';
          });

        runtimePackage = pkgs.symlinkJoin {
          name = "design-editor-runtime-${version}";
          paths = [nativePackage];
          postBuild = ''
            install -Dm644 ${defaultDocument} "$out/share/design-editor/default.de"
          '';
        };

        # Upstream publishes matching Linux archives for tagged releases. These
        # outputs are deliberately separate from the source-built packages: the
        # source build remains the default, while users with a matching release
        # asset can avoid compiling Rust and Skia.
        prebuiltDesktopPackage = pkgs.stdenvNoCC.mkDerivation {
          pname = "design-editor-prebuilt";
          version = releaseVersion;
          src = pkgs.fetchurl {
            url = "https://github.com/kemalcalak/design-editor/releases/download/v${releaseVersion}/design-editor-linux-x86_64.tar.gz";
            hash = releaseManifest.desktopHash;
          };
          dontUnpack = true;
          nativeBuildInputs = [pkgs.autoPatchelfHook pkgs.makeWrapper];
          buildInputs = prebuiltDesktopRuntimeLibraries;
          installPhase = ''
            runHook preInstall
            tar -xzf "$src" -C "$TMPDIR"
            install -Dm755 "$TMPDIR/design-editor" $out/bin/design-editor
            install -Dm644 ${./crates/de-host-desktop/assets/icon.png} \
              $out/share/icons/hicolor/1024x1024/apps/design-editor.png
            install -Dm644 ${pkgs.writeText "design-editor-prebuilt.desktop" ''
              [Desktop Entry]
              Name=Design Editor
              Comment=Open-source AI-native vector design tool
              Exec=design-editor %U
              Terminal=false
              Type=Application
              Icon=design-editor
              Categories=Graphics;
            ''} $out/share/applications/design-editor.desktop
            runHook postInstall
          '';
          postFixup = ''
            wrapProgram $out/bin/design-editor \
              --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath prebuiltDesktopRuntimeLibraries}
          '';
        };

        prebuiltCliPackage = pkgs.stdenvNoCC.mkDerivation {
          pname = "design-editor-cli-prebuilt";
          version = releaseVersion;
          src = pkgs.fetchurl {
            url = "https://github.com/kemalcalak/design-editor/releases/download/v${releaseVersion}/de-cli-linux-x86_64.tar.gz";
            hash = releaseManifest.cliHash;
          };
          dontUnpack = true;
          nativeBuildInputs = [pkgs.autoPatchelfHook];
          buildInputs = prebuiltCliRuntimeLibraries;
          installPhase = ''
            runHook preInstall
            tar -xzf "$src" -C "$TMPDIR"
            install -Dm755 "$TMPDIR/de" $out/bin/de
            runHook postInstall
          '';
        };

        wasmBindgen = rs-harbor.lib.resolveWasmBindgenCli {
          inherit lib pkgs;
          cargoLock = ./Cargo.lock;
        };

        wasmGate = rawName: optimizedName: ''
          wasm_opt_help="$(${pkgs.binaryen}/bin/wasm-opt --help 2>&1 || true)"
          wasm_opt_flags=""
          for flag in --enable-bulk-memory --enable-bulk-memory-opt --enable-nontrapping-float-to-int; do
            if printf '%s\n' "$wasm_opt_help" | grep -qF -- "$flag"; then
              wasm_opt_flags="$wasm_opt_flags $flag"
            fi
          done
          ${pkgs.binaryen}/bin/wasm-opt $wasm_opt_flags -Oz "$out/${rawName}" -o "$out/${optimizedName}"
          cp "$out/${optimizedName}" "$out/${rawName}"
          env_count="$(${pkgs.nodejs}/bin/node -e '
            const fs = require("fs");
            const buf = fs.readFileSync(process.argv[1]);
            WebAssembly.compile(buf).then(mod => {
              console.log(WebAssembly.Module.imports(mod).filter(i => i.module === "env").length);
            }).catch(e => { console.error(e); process.exit(1); });
          ' "$out/${rawName}")"
          test "$env_count" = 0
          gzip_bytes="$(${pkgs.gzip}/bin/gzip -c "$out/${rawName}" | ${pkgs.coreutils}/bin/wc -c)"
          test "$gzip_bytes" -le 6291456
        '';

        webHostBundle = craneLib.buildPackage (commonArgs
          // {
            pname = "design-editor-web-bundle";
            cargoBuildCommand = "cargo build --release --target wasm32-unknown-unknown --no-default-features --features canvaskit --package de-host-web";
            nativeBuildInputs = commonArgs.nativeBuildInputs ++ [wasmBindgen.package pkgs.binaryen pkgs.nodejs pkgs.gzip];
            installPhase = ''
              runHook preInstall
              mkdir -p $out
              ${wasmBindgen.package}/bin/wasm-bindgen --target web --out-dir $out --out-name op_host_web target/wasm32-unknown-unknown/release/de_host_web.wasm
              cp -R crates/de-host-web/assets/canvaskit $out/canvaskit
              ${wasmGate "op_host_web_bg.wasm" "op_host_web_bg.opt.wasm"}
              runHook postInstall
            '';
          });

        webSdkWasm = craneLib.buildPackage (commonArgs
          // {
            pname = "design-editor-web-sdk-wasm";
            cargoBuildCommand = "cargo build --release --target wasm32-unknown-unknown --features canvaskit --package de-web-sdk";
            nativeBuildInputs = commonArgs.nativeBuildInputs ++ [wasmBindgen.package pkgs.binaryen pkgs.nodejs pkgs.gzip];
            installPhase = ''
              runHook preInstall
              mkdir -p $out
              ${wasmBindgen.package}/bin/wasm-bindgen --target web --out-dir $out target/wasm32-unknown-unknown/release/de_web_sdk.wasm
              ${wasmGate "de_web_sdk_bg.wasm" "de_web_sdk_bg.opt.wasm"}
              runHook postInstall
            '';
          });

        webServerPackage = pkgs.symlinkJoin {
          name = "design-editor-${version}";
          paths = [nativePackage];
          postBuild = ''
            mkdir -p $out/bin/web-bundle
            cp -R ${webHostBundle}/. $out/bin/web-bundle/
          '';
        };
        opCliPackage = pkgs.symlinkJoin {
          name = "design-editor-cli-${version}";
          paths = [nativePackage];
          postBuild = ''
            mkdir -p $out/bin
            ln -sf ${nativePackage}/bin/de $out/bin/de
          '';
        };

        defaultDocument = pkgs.writeText "design-editor-default-${version}.de" ''
          {
            "version": "${version}",
            "name": "Design Editor Nix Session",
            "children": []
          }
        '';

        bunToolchain = js-harbor.lib.mkBunToolchain {
          inherit pkgs;
          packageJson = ./packages/package.json;
        };
        bunDeps = js-harbor.lib.mkBunWorkspaceDeps {
          inherit pkgs;
          bun = bunToolchain.bun;
          src = source;
          packageJson = ./packages/package.json;
          lockfile = ./packages/bun.lock;
          version = "1.3.14";
          installFlags = ["--cwd" "packages"];
          # Bun's hoisted aliases are order-dependent when multiple versions
          # are present; nested consumers already point at the correct ones.
          postInstallNormalize = ''
            rm -f packages/node_modules/.bun/node_modules/{string-width,strip-ansi,wrap-ansi}
          '';
          hash = "sha256-aY3Nr1+a1o7TojefRcozmntbdfrF2JRtOrRY8Hqs8rE=";
        };
        webSdkPackages = pkgs.stdenvNoCC.mkDerivation {
          pname = "design-editor-web-sdk-packages";
          inherit version;
          src = source;
          # nodejs_22 (LTS), not pkgs.nodejs: vitest's tinypool 1.x crashes on
          # teardown under Node >= 23 ("emitter.removeListener is not a
          # function" after every test passed), which failed this check on
          # 2026-08-28 twice with all 4 SDK tests green. The wasm derivations
          # keep pkgs.nodejs — only the vitest runner needs the pin.
          nativeBuildInputs = [bunToolchain.bun pkgs.coreutils pkgs.nodejs_22];
          buildPhase = ''
            runHook preBuild
            rm -rf packages/node_modules packages/de-web-sdk/wasm
            cp -R ${bunDeps}/packages/node_modules packages/node_modules
            for workspace in de-web-sdk de-web-sdk-react de-web-sdk-vue; do
              cp -R ${bunDeps}/packages/$workspace/node_modules packages/$workspace/
            done
            chmod -R u+w packages/node_modules
            chmod -R u+w packages/de-web-sdk/node_modules packages/de-web-sdk-react/node_modules packages/de-web-sdk-vue/node_modules
            patchShebangs packages/node_modules
            find packages/node_modules -type f -exec sed -i \
              -e 's|^#!/usr/bin/env bun$|#!${bunToolchain.bun}/bin/bun|' \
              -e 's|^#!/usr/bin/env node$|#!${pkgs.nodejs_22}/bin/node|' {} +
            mkdir -p packages/de-web-sdk/wasm
            cp -R ${webSdkWasm}/. packages/de-web-sdk/wasm/
            bun run --cwd packages sync-version:check
            bun run --cwd packages lint
            bun run --cwd packages/de-web-sdk typecheck
            bun run --cwd packages/de-web-sdk test
            bun run --cwd packages/de-web-sdk build
            bun run --cwd packages/de-web-sdk-react typecheck
            bun run --cwd packages/de-web-sdk-react test
            bun run --cwd packages/de-web-sdk-react build
            bun run --cwd packages/de-web-sdk-vue typecheck
            bun run --cwd packages/de-web-sdk-vue test
            bun run --cwd packages/de-web-sdk-vue build
            runHook postBuild
          '';
          installPhase = ''
            runHook preInstall
            mkdir -p $out
            (cd packages/de-web-sdk && bun pm pack --destination $out)
            (cd packages/de-web-sdk-react && bun pm pack --destination $out)
            (cd packages/de-web-sdk-vue && bun pm pack --destination $out)
            runHook postInstall
          '';
        };

        pythonEnv = py-harbor.lib.mkPythonEnv {inherit pkgs;};
        pythonCheck =
          pkgs.runCommand "design-editor-python-check" {
            nativeBuildInputs = [pythonEnv];
          } ''
            cp -R ${source} "$TMPDIR/design-editor-source"
            chmod -R u+w "$TMPDIR/design-editor-source"
            cd "$TMPDIR/design-editor-source"
            python -m compileall -q scripts tools
            touch $out
          '';

        # Exercise the same `nix run` surface that users consume, inside a
        # clean NixOS VM rather than only evaluating the derivations.
        prebuiltTestFlake = pkgs.writeTextDir "flake.nix" ''
          {
            outputs = {self}: {
              apps.x86_64-linux.prebuilt = {
                type = "app";
                program = "${prebuiltDesktopPackage}/bin/design-editor";
              };
              apps.x86_64-linux.prebuilt-cli = {
                type = "app";
                program = "${prebuiltCliPackage}/bin/de";
              };
            };
          }
        '';

        prebuiltRuntimeTest = pkgs.testers.runNixOSTest ({...}: {
          name = "design-editor-prebuilt-runtime";
          nodes.machine = {pkgs, ...}: {
            system.stateVersion = "25.11";
            virtualisation.memorySize = 2048;
            nix.settings.experimental-features = ["nix-command" "flakes"];
            environment.etc."design-editor-test-flake".source = prebuiltTestFlake;
            environment.variables = {
              LIBGL_ALWAYS_SOFTWARE = "1";
              MESA_LOADER_DRIVER_OVERRIDE = "llvmpipe";
              GALLIUM_DRIVER = "llvmpipe";
            };
            environment.systemPackages = [
              prebuiltDesktopPackage
              prebuiltCliPackage
              pkgs.xvfb-run
            ];
          };
          testScript = ''
            machine.succeed("nix run --offline /etc/design-editor-test-flake#prebuilt-cli -- --version | grep -F ${releaseVersion}")
            machine.succeed("set +e; timeout 15s xvfb-run -a -s '-screen 0 1280x720x24' nix run --offline /etc/design-editor-test-flake#prebuilt >/tmp/design-editor.log 2>&1; rc=$?; test $rc -eq 0 -o $rc -eq 124; ! grep -E 'error while loading|cannot open shared object' /tmp/design-editor.log")
          '';
        });

        runtimePrebuiltPackage = pkgs.symlinkJoin {
          name = "design-editor-runtime-prebuilt-${version}";
          paths = [prebuiltDesktopPackage prebuiltCliPackage];
          nativeBuildInputs = [pkgs.makeWrapper];
          postBuild = ''
            rm -f "$out/bin/de"
            makeWrapper ${prebuiltCliPackage}/bin/de "$out/bin/de" \
              --set-default DESIGN_EDITOR_DESKTOP_BIN "$out/bin/design-editor"
            install -Dm644 ${defaultDocument} "$out/share/design-editor/default.de"
          '';
        };

        runtimePrebuiltToolsCheck = pkgs.runCommand "design-editor-runtime-prebuilt-tools-${version}" {} ''
          test -x ${runtimePrebuiltPackage}/bin/design-editor
          test -x ${runtimePrebuiltPackage}/bin/de
          touch "$out"
        '';

        webServerToolsCheck = pkgs.runCommand "design-editor-web-server-tools-${version}" {} ''
          test -x ${webServerPackage}/bin/de-host-web-server
          test -d ${webServerPackage}/bin/web-bundle
          touch "$out"
        '';

        skillBundle = builtins.fromJSON (
          builtins.replaceStrings
          ["__DESIGN_EDITOR_VERSION__"]
          [version]
          (builtins.readFile ./crates/de-cli/assets/skill-bundle.json)
        );
        skillBundleFiles =
          builtins.mapAttrs
          (relativePath: contents:
            pkgs.writeText
            "design-editor-skill-${builtins.replaceStrings ["/"] ["-"] relativePath}"
            contents)
          skillBundle.files;
        skillsPackage = pkgs.runCommand "design-editor-skills-${version}" {} ''
          mkdir -p "$out/share/skillnet/design-editor"
          install -Dm644 ${./nix/integration/Skillnet.pkl} \
            "$out/share/skillnet/design-editor/Skillnet.pkl"
          ${lib.concatStringsSep "\n" (
            lib.mapAttrsToList
            (relativePath: source: "install -Dm644 ${source} \"$out/share/skillnet/design-editor/${relativePath}\"")
            skillBundleFiles
          )}
        '';

        appimage = rs-harbor.lib.mkAppImage {
          inherit nix-appimage system version;
          pname = "design-editor";
          program = "${nativePackage}/bin/design-editor";
        };
      in {
        packages = {
          default = webServerPackage;
          design-editor = webServerPackage;
          de-cli = opCliPackage;
          runtime = runtimePackage;
          runtime-prebuilt = runtimePrebuiltPackage;
          prebuilt = prebuiltDesktopPackage;
          prebuilt-cli = prebuiltCliPackage;
          skills = skillsPackage;
          web-server = webServerPackage;
          web-bundle = webHostBundle;
          web-sdk-wasm = webSdkWasm;
          web-sdk-packages = webSdkPackages;
          appimage = appimage;
          python-tools = pythonCheck;
        };

        apps = {
          default = {
            type = "app";
            program = "${nativePackage}/bin/design-editor";
          };
          de = {
            type = "app";
            program = "${nativePackage}/bin/de";
          };
          prebuilt = {
            type = "app";
            program = "${prebuiltDesktopPackage}/bin/design-editor";
          };
          prebuilt-cli = {
            type = "app";
            program = "${prebuiltCliPackage}/bin/de";
          };
          web-server = {
            type = "app";
            program = "${webServerPackage}/bin/de-host-web-server";
          };
          integration-export = {
            type = "app";
            program = "${pkgs.writeShellApplication {
              name = "design-editor-integration-export";
              runtimeInputs = [nix-pklx.packages.${system}.pklx pkgs.coreutils];
              text = ''
                export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
                output="''${1:-nix/integration/design-editor.nix}"
                tmp="$(mktemp)"
                trap 'rm -f "$tmp"' EXIT
                pklx eval nix/integration/DesignEditor.pkl -o "$tmp"
                mv "$tmp" "$output"
                echo "Wrote $output from nix/integration/DesignEditor.pkl"
              '';
            }}/bin/design-editor-integration-export";
          };
        };

        devShells.default = pkgs.mkShell {
          packages = [
            toolchain.rustToolchain
            bunToolchain.bun
            pythonEnv
            wasmBindgen.package
            pkgs.binaryen
            pkgs.nodejs
            pkgs.gzip
            pkgs.cargo-watch
            pkgs.chromium
          ];
          nativeBuildInputs = rustNativeInputs;
          buildInputs = nativeLibraries ++ [pkgs.openssl];
          env = nativeEnv;
        };

        formatter = pkgs.alejandra;

        checks = {
          default = webServerPackage;
          native = nativePackage;
          web-bundle = webHostBundle;
          web-sdk-wasm = webSdkWasm;
          web-sdk-packages = webSdkPackages;
          python = pythonCheck;
          prebuilt = prebuiltDesktopPackage;
          prebuilt-cli = prebuiltCliPackage;
          prebuilt-runtime = prebuiltRuntimeTest;
          prebuilt-runtime-tools = runtimePrebuiltToolsCheck;
          web-server-tools = webServerToolsCheck;
          integration-manifest =
            pkgs.runCommand "design-editor-integration-manifest" {
              nativeBuildInputs = [nix-pklx.packages.${system}.pklx];
              SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
            } ''
              pklx eval ${./nix/integration/DesignEditor.pkl} -o "$out"
            '';
          integration-sidecar =
            pkgs.runCommand "design-editor-integration-sidecar" {
              nativeBuildInputs = [nix-pklx.packages.${system}.pklx pkgs.diffutils];
              SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
            } ''
              generated="$TMPDIR/design-editor.nix"
              pklx eval ${./nix/integration/DesignEditor.pkl} -o "$generated"
              diff -u ${./nix/integration/design-editor.nix} "$generated"
              touch "$out"
            '';
          flake-format = pkgs.runCommand "design-editor-flake-format" {} ''
            ${pkgs.alejandra}/bin/alejandra --check ${./flake.nix}
            touch $out
          '';
        };
      };
    };
}
