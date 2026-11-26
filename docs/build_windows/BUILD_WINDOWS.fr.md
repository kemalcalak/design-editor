
> [English](./BUILD_WINDOWS.md) · [简体中文](./BUILD_WINDOWS.zh.md) · [繁體中文](./BUILD_WINDOWS.zh-TW.md) · [日本語](./BUILD_WINDOWS.ja.md) · [한국어](./BUILD_WINDOWS.ko.md) · **Français** · [Español](./BUILD_WINDOWS.es.md) · [Deutsch](./BUILD_WINDOWS.de.md) · [Português](./BUILD_WINDOWS.pt.md) · [Русский](./BUILD_WINDOWS.ru.md) · [हिन्दी](./BUILD_WINDOWS.hi.md) · [Türkçe](./BUILD_WINDOWS.tr.md) · [ไทย](./BUILD_WINDOWS.th.md) · [Tiếng Việt](./BUILD_WINDOWS.vi.md) · [Bahasa Indonesia](./BUILD_WINDOWS.id.md)

# Guide de compilation Design Editor pour Windows

## 1. Téléchargement du code source

1. Téléchargez le code source du dépôt design-editor

2. design-editor possède 3 sous-modules de dépendances dans le répertoire `vendor` (jian, casement, agent) qui doivent être téléchargés séparément

    - 2.1 jian (framework UI) : [https://github.com/ZSeven-W/jian](https://github.com/ZSeven-W/jian) → extraire dans `vendor\jian`

    - 2.2 casement (wrapper de fenêtre) : [https://github.com/ZSeven-W/casement](https://github.com/ZSeven-W/casement) → extraire dans `vendor\casement`

    - 2.3 agent (runtime Agent) : [https://github.com/ZSeven-W/agent-rs](https://github.com/ZSeven-W/agent-rs) → extraire dans `vendor\agent`

3. Après extraction, vérifiez que les chemins suivants existent :

    ```plaintext
    vendor\jian\Cargo.toml
    vendor\casement\Cargo.toml
    vendor\agent\Cargo.toml
    ```

## 2. Préparation de l'environnement — *Windows 10 x64 (chaîne d'outils MSVC)*

1. Installez Visual Studio en cochant **« Développement de bureau avec C++ »**

2. Installez la chaîne d'outils Rust `stable-x86_64-pc-windows-msvc`

    - 2.1 Configurez le miroir rustup (pour accélérer les téléchargements)

        ```powershell
        [Environment]::SetEnvironmentVariable("RUSTUP_DIST_SERVER", "https://rsproxy.cn", "User")
        [Environment]::SetEnvironmentVariable("RUSTUP_UPDATE_ROOT", "https://rsproxy.cn/rustup", "User")
        ```

    - 2.2 Configurez le miroir des dépendances Cargo. Ouvrez votre répertoire utilisateur (`C:\Users\<username>\`), trouvez ou créez le dossier `.cargo`, puis créez un fichier `config.toml` à l'intérieur.

        Collez le contenu suivant dans `C:\Users\<username>\.cargo\config.toml` :

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

    - 2.3 Téléchargez `rustup-init.exe` depuis rustup.rs et exécutez-le. Dans PowerShell, lancez les commandes suivantes :

        ```powershell
        rustup toolchain install 1.94
        rustup default 1.94-x86_64-pc-windows-msvc
        ```

## 3. Compilation

1. À la racine du dépôt, ouvrez PowerShell et exécutez :

    ```powershell
    cargo build --workspace --release
    ```

    > Vous pouvez rencontrer l'erreur `error: failed to run custom build command for skia-bindings v0.97.2`
    
    - 1.1 Téléchargez manuellement le paquet avec backend GL : [skia-binaries-0.97.2 (GL)](https://github.com/rust-skia/skia-binaries/releases/download/0.97.2/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz)
    
    - 1.2 Définissez la variable d'environnement (remplacez par votre chemin réel) :

         ```powershell
        $env:SKIA_BINARIES_URL="file:///path/to/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz"
        ```
    
    - 1.3 Ré-exécutez l'étape 3.1 pour recompiler

## 4. Configuration des variables d'environnement pour le plugin dsh-design-editor

1. Exécutez PowerShell en tant qu'administrateur et lancez les commandes suivantes (remplacez le chemin par la racine réelle du dépôt) :

    ```powershell
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    ```

## 5. Vérification

1. design-editor.exe

    ```powershell
    cd target\release
    ./design-editor.exe --mcp-http 3100 demo.de
    # La fenêtre apparaît brièvement puis se ferme — le service écoute sur le port 3100 en arrière-plan
    ```

---

# Dépannage

1. La chaîne d'outils rustup `1.94` et skia-bindings `0.97.2` correspondent aux versions fixées dans `rust-toolchain.toml` et `Cargo.lock`. Si ces fichiers indiquent une autre version, utilisez la leur.
