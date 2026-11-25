
> [English](./BUILD_WINDOWS.md) · [简体中文](./BUILD_WINDOWS.zh.md) · [繁體中文](./BUILD_WINDOWS.zh-TW.md) · [日本語](./BUILD_WINDOWS.ja.md) · [한국어](./BUILD_WINDOWS.ko.md) · [Français](./BUILD_WINDOWS.fr.md) · [Español](./BUILD_WINDOWS.es.md) · [Deutsch](./BUILD_WINDOWS.de.md) · [Português](./BUILD_WINDOWS.pt.md) · [Русский](./BUILD_WINDOWS.ru.md) · [हिन्दी](./BUILD_WINDOWS.hi.md) · **Türkçe** · [ไทย](./BUILD_WINDOWS.th.md) · [Tiếng Việt](./BUILD_WINDOWS.vi.md) · [Bahasa Indonesia](./BUILD_WINDOWS.id.md)

# Design Editor Windows Derleme Kılavuzu

## 1. Kaynak kodunu indirme

1. design-editor deposunun kaynak kodunu indirin

2. design-editor, `vendor` dizininde 3 bağımlılık alt modülü (jian, casement, agent) içerir ve bunları ayrı ayrı indirmeniz gerekir

    - 2.1 jian (UI çerçevesi): [https://github.com/ZSeven-W/jian](https://github.com/ZSeven-W/jian) → `vendor\jian` dizinine çıkarın

    - 2.2 casement (pencere sarmalayıcı): [https://github.com/ZSeven-W/casement](https://github.com/ZSeven-W/casement) → `vendor\casement` dizinine çıkarın

    - 2.3 agent (Agent çalışma zamanı): [https://github.com/ZSeven-W/agent-rs](https://github.com/ZSeven-W/agent-rs) → `vendor\agent` dizinine çıkarın

3. Çıkardıktan sonra aşağıdaki yolların mevcut olduğundan emin olun:

    ```plaintext
    vendor\jian\Cargo.toml
    vendor\casement\Cargo.toml
    vendor\agent\Cargo.toml
    ```

## 2. Ortam hazırlığı — *Windows 10 x64 (MSVC araç zinciri)*

1. Visual Studio'yu **"C++ ile Masaüstü Geliştirme"** iş yükünü seçerek yükleyin

2. Rust araç zincirini `stable-x86_64-pc-windows-msvc` olarak yükleyin

    - 2.1 Rustup aynasını yapılandırın (indirmeleri hızlandırmak için)

        ```powershell
        [Environment]::SetEnvironmentVariable("RUSTUP_DIST_SERVER", "https://rsproxy.cn", "User")
        [Environment]::SetEnvironmentVariable("RUSTUP_UPDATE_ROOT", "https://rsproxy.cn/rustup", "User")
        ```

    - 2.2 Cargo bağımlılık aynasını yapılandırın. Kullanıcı dizininizi (`C:\Users\<username>\`) açın, `.cargo` klasörünü bulun veya oluşturun ve içine `config.toml` dosyası oluşturun.

        `C:\Users\<username>\.cargo\config.toml` dosyasına aşağıdaki içeriği yapıştırın:

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

    - 2.3 rustup.rs adresinden `rustup-init.exe` dosyasını indirin ve çalıştırın. PowerShell'de aşağıdaki komutları çalıştırın:

        ```powershell
        rustup toolchain install 1.94
        rustup default 1.94-x86_64-pc-windows-msvc
        ```

## 3. Derleme

1. Depo kök dizininde PowerShell'i açın ve çalıştırın:

    ```powershell
    cargo build --workspace --release
    ```

    > `error: failed to run custom build command for skia-bindings v0.97.2` hatası alabilirsiniz
    
    - 1.1 GL arka uçlu paketi manuel olarak indirin: [skia-binaries-0.97.2 (GL)](https://github.com/rust-skia/skia-binaries/releases/download/0.97.2/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz)
    
    - 1.2 Ortam değişkenini ayarlayın (gerçek yolunuzla değiştirin):

         ```powershell
        $env:SKIA_BINARIES_URL="file:///path/to/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz"
        ```
    
    - 1.3 Yeniden derlemek için 3.1 adımını tekrar çalıştırın

## 4. dsh-design-editor eklentisi için ortam değişkenlerini yapılandırma

1. Yönetici olarak PowerShell'i çalıştırın ve aşağıdaki komutları çalıştırın (yolu gerçek depo kök dizinyle değiştirin):

    ```powershell
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    ```

## 5. Doğrulama

1. design-editor.exe

    ```powershell
    cd target\release
    ./design-editor.exe --mcp-http 3100 demo.de
    # Pencere kısa süre görünür ve kapanır — arka planda 3100 portunu dinleyen servis çalışır
    ```

---

# Sorun giderme

1. Rustup araç zinciri `1.94` ve skia-bindings `0.97.2`, `rust-toolchain.toml` ile `Cargo.lock` dosyalarında sabitlenen sürümlerdir. Bu dosyalarda farklı bir sürüm yazıyorsa onu kullanın.
