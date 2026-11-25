
> [English](./BUILD_WINDOWS.md) · [简体中文](./BUILD_WINDOWS.zh.md) · [繁體中文](./BUILD_WINDOWS.zh-TW.md) · [日本語](./BUILD_WINDOWS.ja.md) · [한국어](./BUILD_WINDOWS.ko.md) · [Français](./BUILD_WINDOWS.fr.md) · [Español](./BUILD_WINDOWS.es.md) · [Deutsch](./BUILD_WINDOWS.de.md) · [Português](./BUILD_WINDOWS.pt.md) · [Русский](./BUILD_WINDOWS.ru.md) · [हिन्दी](./BUILD_WINDOWS.hi.md) · [Türkçe](./BUILD_WINDOWS.tr.md) · [ไทย](./BUILD_WINDOWS.th.md) · [Tiếng Việt](./BUILD_WINDOWS.vi.md) · **Bahasa Indonesia**

# Panduan Build Design Editor untuk Windows

## 1. Mengunduh Sumber Kode

1. Unduh sumber kode dari repositori design-editor

2. design-editor memiliki 3 submodul dependensi di direktori `vendor` (jian, casement, agent) yang perlu diunduh secara terpisah

    - 2.1 jian (framework UI): [https://github.com/ZSeven-W/jian](https://github.com/ZSeven-W/jian) → ekstrak ke `vendor\jian`

    - 2.2 casement (pembungkus window): [https://github.com/ZSeven-W/casement](https://github.com/ZSeven-W/casement) → ekstrak ke `vendor\casement`

    - 2.3 agent (runtime Agent): [https://github.com/ZSeven-W/agent-rs](https://github.com/ZSeven-W/agent-rs) → ekstrak ke `vendor\agent`

3. Setelah ekstraksi, pastikan path berikut ada:

    ```plaintext
    vendor\jian\Cargo.toml
    vendor\casement\Cargo.toml
    vendor\agent\Cargo.toml
    ```

## 2. Persiapan Lingkungan — *Windows 10 x64 (toolchain MSVC)*

1. Instal Visual Studio dengan memilih **"Development with C++ for Desktop"**

2. Instal toolchain Rust `stable-x86_64-pc-windows-msvc`

    - 2.1 Konfigurasi mirror rustup (untuk mempercepat unduhan)

        ```powershell
        [Environment]::SetEnvironmentVariable("RUSTUP_DIST_SERVER", "https://rsproxy.cn", "User")
        [Environment]::SetEnvironmentVariable("RUSTUP_UPDATE_ROOT", "https://rsproxy.cn/rustup", "User")
        ```

    - 2.2 Konfigurasi mirror dependensi Cargo. Buka direktori pengguna (`C:\Users\<username>\`), temukan atau buat folder `.cargo`, lalu buat file `config.toml` di dalamnya.

        Tempelkan konten berikut ke `C:\Users\<username>\.cargo\config.toml`:

        ```toml
        [source.crates-io]
        replace-with = 'rsproxy-sparse'

        [source.rsproxy]
        registry = "https://rsproxy.cn/crates.io-index"

        [source.rsproxy-sparse]
        registry = "sparse+https://rsproxy.cn/index/"

        [registries.rsproxy]
        index = "https://rsproxy.cn/crates.io-index"

        [net]
        git-fetch-with-cli = true
        ```

    - 2.3 Unduh `rustup-init.exe` dari rustup.rs dan jalankan. Eksekusi perintah berikut di PowerShell:

        ```powershell
        rustup toolchain install 1.94
        rustup default 1.94-x86_64-pc-windows-msvc
        ```

## 3. Build

1. Di root repositori, buka PowerShell dan eksekusi:

    ```powershell
    cargo build --workspace --release
    ```

    > Anda mungkin menemui error `error: failed to run custom build command for skia-bindings v0.97.2`
    
    - 1.1 Unduh manual paket dengan backend GL: [skia-binaries-0.97.2 (GL)](https://github.com/rust-skia/skia-binaries/releases/download/0.97.2/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz)
    
    - 1.2 Atur environment variable (ganti dengan path Anda yang sebenarnya):

         ```powershell
        $env:SKIA_BINARIES_URL="file:///path/to/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz"
        ```
    
    - 1.3 Jalankan lagi langkah 3.1 untuk kompilasi ulang

## 4. Konfigurasi Environment Variable untuk Plugin dsh-design-editor

1. Jalankan PowerShell sebagai administrator dan eksekusi perintah berikut (ganti path dengan root repositori yang sebenarnya):

    ```powershell
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    ```

## 5. Verifikasi

1. design-editor.exe

    ```powershell
    cd target\release
    ./design-editor.exe --mcp-http 3100 demo.de
    # Jendela muncul sebentar lalu menutup — layanan berjalan di latar belakang dan mendengarkan port 3100
    ```

---

# Pemecahan Masalah

1. Toolchain rustup `1.94` dan skia-bindings `0.97.2` sesuai dengan versi yang dipatok di `rust-toolchain.toml` dan `Cargo.lock`. Jika berkas tersebut menyebut versi lain, gunakan versi itu.
