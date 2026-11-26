
> [**English**](./BUILD_WINDOWS.md) · [简体中文](./BUILD_WINDOWS.zh.md) · [繁體中文](./BUILD_WINDOWS.zh-TW.md) · [日本語](./BUILD_WINDOWS.ja.md) · [한국어](./BUILD_WINDOWS.ko.md) · [Français](./BUILD_WINDOWS.fr.md) · [Español](./BUILD_WINDOWS.es.md) · [Deutsch](./BUILD_WINDOWS.de.md) · [Português](./BUILD_WINDOWS.pt.md) · [Русский](./BUILD_WINDOWS.ru.md) · [हिन्दी](./BUILD_WINDOWS.hi.md) · [Türkçe](./BUILD_WINDOWS.tr.md) · [ไทย](./BUILD_WINDOWS.th.md) · [Tiếng Việt](./BUILD_WINDOWS.vi.md) · [Bahasa Indonesia](./BUILD_WINDOWS.id.md)

# Design Editor Windows Build Guide

## 1. Download Source Code

1. **Option A (recommended):** Clone with submodules automatically:

    ```bash
    git clone --recurse-submodules https://github.com/kemalcalak/design-editor.git
    ```

    > If already cloned without submodules, run from the repository root:
    >
    > ```bash
    > git submodule update --init --recursive
    > ```

    **Or Option B:** Manually download and extract the following 3 repositories:

    - jian (UI framework): [https://github.com/ZSeven-W/jian](https://github.com/ZSeven-W/jian) → extract to `vendor\jian`

    - casement (window wrapper): [https://github.com/ZSeven-W/casement](https://github.com/ZSeven-W/casement) → extract to `vendor\casement`

    - agent (Agent runtime): [https://github.com/ZSeven-W/agent-rs](https://github.com/ZSeven-W/agent-rs) → extract to `vendor\agent`

2. Confirm the 3 submodules under `vendor` are ready:

    ```plaintext
    vendor\jian\Cargo.toml
    vendor\casement\Cargo.toml
    vendor\agent\Cargo.toml
    ```

## 2. Environment Setup — *Windows 10 x64 (MSVC Toolchain)*

1. Install Visual Studio with the **"Desktop development with C++"** workload checked

2. Install the Rust toolchain `stable-x86_64-pc-windows-msvc`:

    - 2.1 Download `rustup-init.exe` from rustup.rs and run it. Execute the following in PowerShell:

        ```powershell
        rustup toolchain install 1.94
        rustup default 1.94-x86_64-pc-windows-msvc
        ```

## 3. Build

1. In the repository root, open PowerShell and run:

    ```powershell
    cargo build --workspace --release
    ```

    > You may encounter `error: failed to run custom build command for skia-bindings v0.97.2`
    
    - 1.1 Manually download the GL-backend package: [skia-binaries-0.97.2 (GL)](https://github.com/rust-skia/skia-binaries/releases/download/0.97.2/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz)
    
    - 1.2 Set the environment variable (replace with your actual path):
    
         ```powershell
        $env:SKIA_BINARIES_URL="file:///path/to/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz"
        ```
    
    - 1.3 Re-run step 3.1 to rebuild

## 4. Configure Environment Variables for the dsh-design-editor Plugin

1. Run PowerShell as Administrator and execute the following (replace the path with your actual repository root):

    ```powershell
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    ```

## 5. Verification

1. design-editor.exe

    ```powershell
    cd target\release
    ./design-editor.exe --mcp-http 3100 demo.de
    # The window flashes and closes — the service runs in the background on port 3100
    ```

---

# Notes

1. The rustup toolchain `1.94` and skia-bindings `0.97.2` match the versions pinned in `rust-toolchain.toml` and `Cargo.lock`. If those files name a different version, use theirs.
