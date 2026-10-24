#[test]
fn rust_check_runs_canvaskit_widget_host_tests() {
    let workflow = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/rust-check.yml"
    ))
    .expect("rust-check workflow is readable");

    assert!(
        workflow.contains("Run CanvasKit web host tests"),
        "rust-check should name the CanvasKit web host test step"
    );
    assert!(
        workflow.contains("cargo test -p de-host-web"),
        "rust-check should run de-host-web tests explicitly"
    );
    assert!(
        workflow.contains("--features canvaskit"),
        "rust-check must enable the production CanvasKit web host feature"
    );
}

#[test]
fn macos_bundle_signing_uses_hardened_runtime_for_developer_id() {
    let script = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/bundle-macos.sh"
    ))
    .expect("bundle-macos script is readable");

    assert!(
        script.contains("--timestamp --options runtime"),
        "Developer ID macOS signing should enable hardened runtime and secure timestamp"
    );
    assert!(
        script.contains("sign_macos_code \"$APP/Contents/MacOS/design-editor\""),
        "bundle script should explicitly sign the main app executable before signing the bundle"
    );
    assert!(
        script.contains("sign_macos_code \"$APP/Contents/MacOS/de\""),
        "bundle script should sign the embedded de CLI before signing the bundle"
    );

    let sign_cli = script
        .find("sign_macos_code \"$APP/Contents/MacOS/de\"")
        .expect("bundle script signs embedded de CLI");
    let sign_main = script
        .find("sign_macos_code \"$APP/Contents/MacOS/design-editor\"")
        .expect("bundle script signs the main app executable");
    assert!(
        sign_cli < sign_main,
        "bundle script should sign embedded CLI executables before the main app executable"
    );
}

#[test]
fn release_workflow_installs_nsis_on_windows_runner() {
    let workflow = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/rust-release.yml"
    ))
    .expect("rust-release workflow is readable");

    let install_step_start = workflow
        .find("- name: Install NSIS (windows)")
        .expect("release workflow should install NSIS before invoking makensis");
    let following_step_offset = workflow[install_step_start..]
        .find("- name: Stage ANGLE DLLs (windows)")
        .expect("NSIS installation should remain a dedicated release step");
    let install_step = &workflow[install_step_start..install_step_start + following_step_offset];
    let self_test = install_step
        .find("& tools/install-pinned-nsis.ps1 -SelfTest")
        .expect("the digest-pinned NSIS installer should reject checksum mismatches first");
    let install = install_step
        .find("\n          & tools/install-pinned-nsis.ps1\n")
        .expect("the verified NSIS installer should run after its self-test");

    assert!(
        self_test < install,
        "the checksum self-test must pass before the digest-pinned NSIS install"
    );
    assert!(
        !workflow.contains("choco install"),
        "release workflow must not regress to an unpinned Chocolatey install"
    );
}

#[test]
fn release_workflow_services_pinned_vc_runtime_for_nsis() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workflow = std::fs::read_to_string(repo.join(".github/workflows/rust-release.yml"))
        .expect("rust-release workflow is readable");
    assert!(
        workflow
            .find("& tools/stage-pinned-vcredist.ps1 -ValidateBuildToolset")
            .expect("release workflow should run the MSVC compatibility gate")
            < workflow
                .find("- name: Build desktop app and CLI")
                .expect("release workflow should build the host"),
        "the Windows MSVC compatibility gate must run before the host build"
    );
    let stage_start = workflow
        .find("- name: Stage pinned Microsoft Visual C++ Redistributable (windows)")
        .expect("release workflow should stage the pinned VC++ Redistributable");
    let package_start = workflow
        .find("- name: Package NSIS installer (windows)")
        .expect("release workflow should package the NSIS installer");
    assert!(
        stage_start < package_start,
        "the verified VC++ Redistributable must be staged before makensis runs"
    );
    let stage_step = &workflow[stage_start..package_start];
    assert!(
        stage_step.contains("& tools/stage-pinned-vcredist.ps1 -SelfTest")
            && stage_step.contains("& tools/stage-pinned-vcredist.ps1 -Destination $vcRedist"),
        "the Windows release must run the network-free stager self-test before staging its payload"
    );
    assert!(
        stage_step.contains("VC_REDIST_FILE=$vcRedist") && stage_step.contains("$env:GITHUB_ENV"),
        "the staged VC++ Redistributable path must cross the PowerShell step boundary explicitly"
    );
    let package_step = &workflow[package_start..];
    assert!(
        package_step.contains("makensis")
            && package_step.contains(r#"/DVC_REDIST_FILE=$env:VC_REDIST_FILE"#),
        "makensis must receive the staged VC++ Redistributable as a mandatory input"
    );
}

/// The in-app update check downloads `asset_name_for(version)`
/// (crates/de-host-desktop/src/update_check.rs) from the release, so the
/// workflow must publish exactly those names.
#[test]
fn release_workflow_publishes_the_update_check_asset_names() {
    let workflow = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/rust-release.yml"
    ))
    .expect("rust-release workflow is readable");
    for needle in [
        "Design-Editor-$DE_VERSION-${{ matrix.arch }}-mac.dmg",
        "Design-Editor-$env:DE_VERSION-${{ matrix.arch }}-win-setup.exe",
        "\"Design-Editor-$DE_VERSION-arm64-mac.dmg\"",
        "\"Design-Editor-$DE_VERSION-x64-win-setup.exe\"",
        "arch: arm64",
        "arch: x64",
    ] {
        assert!(workflow.contains(needle), "release workflow lost {needle}");
    }
}

/// Every release ships two Linux archives (bare binaries at the archive
/// root), so the release must build, package and require exactly those names.
#[test]
fn release_workflow_publishes_the_linux_archives() {
    let workflow = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/rust-release.yml"
    ))
    .expect("rust-release workflow is readable");
    for asset in [
        "design-editor-linux-x86_64.tar.gz",
        "de-cli-linux-x86_64.tar.gz",
    ] {
        assert!(
            workflow.contains(&format!("            {asset} \\\n"))
                || workflow.contains(&format!("            {asset}; do\n")),
            "the publish step must require {asset}"
        );
    }
    for needle in [
        "label: linux-x86_64",
        "target: x86_64-unknown-linux-gnu",
        "tar czf design-editor-${{ matrix.label }}.tar.gz -C target/${{ matrix.target }}/release design-editor",
        "tar czf de-cli-${{ matrix.label }}.tar.gz -C target/${{ matrix.target }}/release de",
        "            design-editor-${{ matrix.label }}.tar.gz\n",
    ] {
        assert!(workflow.contains(needle), "release workflow lost {needle}");
    }
}

#[test]
fn windows_nsis_installer_services_vc_runtime_fail_closed() {
    let installer = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/package-windows.nsi"
    ))
    .expect("Windows NSIS installer is readable");
    assert!(
        installer.contains("!ifndef VC_REDIST_FILE")
            && installer.contains(r#"File "/oname=${VC_REDIST_EXE}" "${VC_REDIST_FILE}""#)
            && !installer.contains(r#"File /nonfatal "/oname=${VC_REDIST_EXE}""#),
        "the VC++ Redistributable must be a required, embedded NSIS input"
    );
    assert!(
        installer.contains(
            "!getdllversion /packed /productversion \"${VC_REDIST_FILE}\" VC_REDIST_VERSION_",
        ) && !installer.contains("!getdllversion /noerrors")
            && installer.contains("StrCpy $VCRuntimeRequiredHigh \"${VC_REDIST_VERSION_HIGH}\"")
            && installer.contains("StrCpy $VCRuntimeRequiredLow \"${VC_REDIST_VERSION_LOW}\"")
            && installer.contains("SetRegView 64")
            && installer.contains(r#"SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\${ARCH}"#)
            && installer.contains("Installed"),
        "NSIS must compare the embedded Runtime version with the installed machine Runtime"
    );
    assert!(
        installer.contains("ExecWait")
            && installer.contains("/install /passive /norestart")
            && installer.contains("Design-Editor-vc-redist-${ARCH}.log"),
        "NSIS must invoke the Microsoft Runtime installer non-interactively and retain its log"
    );
    assert!(
        installer.contains("SetRebootFlag true")
            && installer.contains("Function .onInstSuccess")
            && installer.contains("SetErrorLevel 3010"),
        "NSIS must preserve reboot-required state and re-check already-installed status"
    );
    assert!(
        installer.contains("MB_ICONSTOP")
            && installer
                .matches("Abort \"Microsoft Visual C++ Redistributable")
                .count()
                == 3,
        "a failed Runtime install or post-install version check must abort Design Editor installation"
    );
}

#[test]
fn windows_cli_installer_services_vc_runtime_fail_closed() {
    let installer = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/install-de.ps1"
    ))
    .expect("Windows CLI installer is readable");
    assert!(
        installer.contains("14.51.36247.0")
            && installer.contains("https://aka.ms/vs/18/release/14.51.36247/VC_redist.x64.exe")
            && installer
                .contains("843068991daaa1f73ad9f6239bce4d0f6a07a51f18c37ea2a867e9beca71295c"),
        "the CLI installer must bind the reviewed VC++ Runtime version, URL, and digest"
    );
    assert!(
        installer.contains("function Get-InstalledVcRedistVersion")
            && installer.contains("[Microsoft.Win32.RegistryView]::Registry64")
            && installer.contains(
                r#"SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\$RuntimeArch"#,
            )
            && installer.contains("$env:PROCESSOR_ARCHITEW6432")
            && installer.contains("$env:PROCESSOR_ARCHITECTURE")
            && installer.contains(r#"$VcRuntimeArch = "arm64""#)
            && installer.contains(r#"$VcRuntimeArch = "x64""#),
        "the CLI installer must query the native-architecture Runtime key through the 64-bit registry view"
    );
    let service_start = installer
        .find("function Install-VcRedistIfRequired")
        .expect("CLI installer should define Runtime servicing");
    let service_end = installer[service_start..]
        .find("function Resolve-Version")
        .map(|offset| service_start + offset)
        .expect("Runtime servicing should be isolated before release resolution");
    let service = &installer[service_start..service_end];
    assert_eq!(
        service.matches("Get-InstalledVcRedistVersion").count(),
        2,
        "Runtime servicing must check the installed version both before and after installation"
    );
    assert!(
        service.contains("$InstallerItem.PSIsContainer")
            && service.contains("[System.IO.FileAttributes]::ReparsePoint")
            && service.contains("$InstallerItem.Length -le 0")
            && service.contains("Get-AuthenticodeSignature")
            && service.contains("[System.Management.Automation.SignatureStatus]::Valid")
            && service.contains("GetNameInfo")
            && service.contains("X509NameType]::SimpleName")
            && service.contains("$SignerName -cne \"Microsoft Corporation\"")
            && service.contains("O=Microsoft Corporation"),
        "the downloaded Runtime must be a regular file with a valid Microsoft Authenticode signer"
    );
    assert!(
        service.contains("Start-Process")
            && service.contains("-Verb RunAs -Wait -PassThru")
            && service.contains("/install /passive /norestart /log")
            && service.contains("@(0, 3010, 1638)"),
        "the Runtime installer must run elevated/passively and accept only reviewed success statuses"
    );
    assert!(
        service.contains("$null -eq $InstalledVersion -or $InstalledVersion -lt $VcRedistVersion")
            && service.contains("throw \"install-de: Visual C++ runtime"),
        "the CLI install must fail closed when the post-install Runtime version is insufficient"
    );
    let service_call = installer
        .find("$VcRedistRebootRequired = Install-VcRedistIfRequired")
        .expect("CLI install should service the Runtime");
    let archive_download = installer
        .find("Invoke-WebRequest -Uri $Url -OutFile $Archive")
        .expect("CLI install should download its release archive");
    assert!(
        service_call < archive_download,
        "the VC++ Runtime prerequisite must be ready before installing the CLI archive"
    );
    let reboot_branch = installer
        .rfind("if ($VcRedistRebootRequired)")
        .expect("CLI installer should branch on Runtime reboot status");
    let reboot_branch = &installer[reboot_branch..];
    assert!(
        reboot_branch
            .find("} else {")
            .zip(reboot_branch.find("& $Target --version"))
            .is_some_and(|(otherwise, verify)| otherwise < verify),
        "a 3010 Runtime result must not execute de before Windows restarts"
    );
}

#[test]
fn web_smoke_page_uses_current_canvaskit_bundle_command() {
    let smoke = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/smoke/step-1b.html"))
        .expect("web smoke page is readable");

    assert!(
        smoke.contains("--features canvaskit"),
        "web smoke page should show the current CanvasKit bundle build command"
    );
    assert!(
        smoke.contains("mod.mount_ck('op')"),
        "web smoke page should mount the production CanvasKit shell"
    );
    assert!(
        !smoke.contains("mod.mount('op')"),
        "web smoke page must not mount the default wasm stub"
    );
    assert!(
        !smoke.contains("--features skia"),
        "web smoke page must not point users at the retired skia wasm build"
    );
    assert!(
        !smoke.contains("EMSDK"),
        "web smoke page must not require the retired EMSDK setup"
    );
}

#[test]
fn web_integration_notes_avoid_legacy_ambiguous_markers() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = [
        "Cargo.toml",
        "src/dom_io.rs",
        "src/file_actions.rs",
        "src/iconify_web.rs",
        "src/lib.rs",
        "src/live_sync.rs",
        "src/raf_pump.rs",
        "src/web_ai_transport.rs",
        "src/web_chat.rs",
        "src/web_clipboard.rs",
        "src/web_fonts.rs",
    ];

    for file in files {
        let text = std::fs::read_to_string(manifest_dir.join(file))
            .unwrap_or_else(|err| panic!("{file} is readable: {err}"));
        assert!(
            !text.contains("UNVERIFIED in-browser"),
            "{file} should name the concrete smoke/daemon boundary instead of a blanket UNVERIFIED marker"
        );
        assert!(
            !text.contains("EMSDK"),
            "{file} should describe the current CanvasKit/web-sys boundary instead of the retired EMSDK path"
        );
    }
}

#[test]
fn browser_smoke_script_and_ci_cover_canvas_and_daemon_paths() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = std::fs::read_to_string(repo.join("tools/check-web-browser-smoke.sh"))
        .expect("browser smoke script is readable");
    let workflow = std::fs::read_to_string(repo.join(".github/workflows/wasm-bundle-build.yml"))
        .expect("wasm bundle workflow is readable");

    assert!(
        script.contains("--dump-dom"),
        "browser smoke should drive a real headless browser, not only curl"
    );
    assert!(
        script.contains("/smoke/step-1b.html"),
        "browser smoke should cover the pure CanvasKit mount harness"
    );
    assert!(
        script.contains("/api/mcp/server"),
        "browser smoke should cover the daemon health path"
    );
    assert!(
        script.contains("data-de-smoke=\"ok\""),
        "browser smoke should assert the DOM success marker written after mount"
    );
    assert!(
        script.contains("local status=$?")
            && script.contains("set +e")
            && script.contains("kill -9 \"$SERVER_PID\"")
            && script.matches("kill -9 \"$chrome_pid\"").count() >= 3
            && script.contains("for _ in $(seq 1 20)")
            && script.contains("return \"$status\""),
        "cleanup should preserve the smoke result instead of replacing it with a profile deletion race"
    );
    assert!(
        workflow.contains("tools/check-web-browser-smoke.sh"),
        "bundle workflow should run the browser smoke after building the bundle"
    );
}

#[test]
fn web_rust_publish_has_docker_and_start_script_entrypoints() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dockerfile =
        std::fs::read_to_string(repo.join("Dockerfile.web-rust")).expect("Dockerfile readable");
    let start_script = std::fs::read_to_string(repo.join("scripts/start-web-rust.sh"))
        .expect("web rust start script is readable");

    assert!(
        dockerfile.contains("de-host-web-server --serve-web"),
        "web Rust Docker image should publish through the headless serve-web daemon"
    );
    assert!(
        start_script.contains("tools/check-wasm-bundle.sh"),
        "start script should build or verify the deployable CanvasKit bundle"
    );
    assert!(
        start_script.contains("de-host-web-server") && start_script.contains("--serve-web"),
        "start script should publish through de-host-web-server --serve-web"
    );
    assert!(
        start_script.contains("DESIGN_EDITOR_WEB_BUNDLE_DIR"),
        "start script should point the daemon at the built web bundle explicitly"
    );
}
