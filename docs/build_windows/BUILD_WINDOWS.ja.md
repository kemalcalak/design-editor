
> [English](./BUILD_WINDOWS.md) · [简体中文](./BUILD_WINDOWS.zh.md) · [繁體中文](./BUILD_WINDOWS.zh-TW.md) · **日本語** · [한국어](./BUILD_WINDOWS.ko.md) · [Français](./BUILD_WINDOWS.fr.md) · [Español](./BUILD_WINDOWS.es.md) · [Deutsch](./BUILD_WINDOWS.de.md) · [Português](./BUILD_WINDOWS.pt.md) · [Русский](./BUILD_WINDOWS.ru.md) · [हिन्दी](./BUILD_WINDOWS.hi.md) · [Türkçe](./BUILD_WINDOWS.tr.md) · [ไทย](./BUILD_WINDOWS.th.md) · [Tiếng Việt](./BUILD_WINDOWS.vi.md) · [Bahasa Indonesia](./BUILD_WINDOWS.id.md)

# Design Editor Windows ビルドガイド

## 1. ソースコードのダウンロード

1. design-editor リポジトリのソースコードをダウンロードします

2. design-editor には `vendor` ディレクトリに 3 つの依存サブモジュール（jian、casement、agent）があり、それぞれ個別にダウンロードする必要があります

    - 2.1 jian（UI フレームワーク）：[https://github.com/ZSeven-W/jian](https://github.com/ZSeven-W/jian) → `vendor\jian` に展開

    - 2.2 casement（ウィンドウラッパー）：[https://github.com/ZSeven-W/casement](https://github.com/ZSeven-W/casement) → `vendor\casement` に展開

    - 2.3 agent（Agent ランタイム）：[https://github.com/ZSeven-W/agent-rs](https://github.com/ZSeven-W/agent-rs) → `vendor\agent` に展開

3. 展開後、以下のパスが存在することを確認：

    ```plaintext
    vendor\jian\Cargo.toml
    vendor\casement\Cargo.toml
    vendor\agent\Cargo.toml
    ```

## 2. 環境準備 — *Windows 10 x64（MSVC ツールチェーン）*

1. Visual Studio をインストールし、**「C++によるデスクトップ開発」** ワークロードを選択

2. Rust ツールチェーン `stable-x86_64-pc-windows-msvc` をインストール

    - 2.1 rustup ミラーの設定（ダウンロード速度の向上）

        ```powershell
        [Environment]::SetEnvironmentVariable("RUSTUP_DIST_SERVER", "https://rsproxy.cn", "User")
        [Environment]::SetEnvironmentVariable("RUSTUP_UPDATE_ROOT", "https://rsproxy.cn/rustup", "User")
        ```

    - 2.2 Cargo 依存関係ミラーの設定。ユーザーディレクトリ（`C:\Users\<username>\`）を開き、`.cargo` フォルダがなければ作成し、その中に `config.toml` ファイルを作成

        `C:\Users\<username>\.cargo\config.toml` に以下の内容を貼り付け：

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

    - 2.3 rustup.rs から `rustup-init.exe` をダウンロードし、ダブルクリックしてインストール。PowerShell で以下を実行：

        ```powershell
        rustup toolchain install 1.94
        rustup default 1.94-x86_64-pc-windows-msvc
        ```

## 3. ビルド

1. リポジトリルートで PowerShell を開き、以下を実行：

    ```powershell
    cargo build --workspace --release
    ```

    > `error: failed to run custom build command for skia-bindings v0.97.2` エラーが発生する場合があります
    
    - 1.1 GL バックエンド付きパッケージを手動ダウンロード：[skia-binaries-0.97.2 (GL)](https://github.com/rust-skia/skia-binaries/releases/download/0.97.2/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz)
    
    - 1.2 環境変数を設定（実際のパスに置き換え）：

         ```powershell
        $env:SKIA_BINARIES_URL="file:///path/to/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz"
        ```
    
    - 1.3 ステップ 3.1 を再実行してリビルド

## 4. dsh-design-editor プラグインの環境変数設定

1. 管理者として PowerShell を実行し、以下を実行（パスを実際のリポジトリルートに置き換え）：

    ```powershell
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    ```

## 5. 検証

1. design-editor.exe

    ```powershell
    cd target\release
    ./design-editor.exe --mcp-http 3100 demo.de
    # ウィンドウが一瞬表示されて閉じます — バックグラウンドで3100ポートをリッスンします
    ```

---

# トラブルシューティング

1. rustup ツールチェーン `1.94`、skia-bindings `0.97.2` は `rust-toolchain.toml` と `Cargo.lock` で固定されたバージョンに対応しています。これらのファイルが別のバージョンを指定している場合は、そちらを使用してください。
