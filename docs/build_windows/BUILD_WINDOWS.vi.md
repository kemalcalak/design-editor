
> [English](./BUILD_WINDOWS.md) · [简体中文](./BUILD_WINDOWS.zh.md) · [繁體中文](./BUILD_WINDOWS.zh-TW.md) · [日本語](./BUILD_WINDOWS.ja.md) · [한국어](./BUILD_WINDOWS.ko.md) · [Français](./BUILD_WINDOWS.fr.md) · [Español](./BUILD_WINDOWS.es.md) · [Deutsch](./BUILD_WINDOWS.de.md) · [Português](./BUILD_WINDOWS.pt.md) · [Русский](./BUILD_WINDOWS.ru.md) · [हिन्दी](./BUILD_WINDOWS.hi.md) · [Türkçe](./BUILD_WINDOWS.tr.md) · [ไทย](./BUILD_WINDOWS.th.md) · **Tiếng Việt** · [Bahasa Indonesia](./BUILD_WINDOWS.id.md)

# Hướng dẫn biên dịch Design Editor cho Windows

## 1. Tải mã nguồn

1. Tải mã nguồn từ kho lưu trữ design-editor

2. design-editor có 3 mô-đun phụ thuộc trong thư mục `vendor` (jian, casement, agent) cần tải riêng

    - 2.1 jian (khung UI): [https://github.com/ZSeven-W/jian](https://github.com/ZSeven-W/jian) → giải nén vào `vendor\jian`

    - 2.2 casement (bọc cửa sổ): [https://github.com/ZSeven-W/casement](https://github.com/ZSeven-W/casement) → giải nén vào `vendor\casement`

    - 2.3 agent (thời gian chạy Agent): [https://github.com/ZSeven-W/agent-rs](https://github.com/ZSeven-W/agent-rs) → giải nén vào `vendor\agent`

3. Sau khi giải nén, xác nhận các đường dẫn sau tồn tại:

    ```plaintext
    vendor\jian\Cargo.toml
    vendor\casement\Cargo.toml
    vendor\agent\Cargo.toml
    ```

## 2. Chuẩn bị môi trường — *Windows 10 x64 (chuỗi công cụ MSVC)*

1. Cài đặt Visual Studio và chọn **"Phát triển ứng dụng Desktop bằng C++"**

2. Cài đặt chuỗi công cụ Rust `stable-x86_64-pc-windows-msvc`

    - 2.1 Cấu hình gương rustup (để tăng tốc tải xuống)

        ```powershell
        [Environment]::SetEnvironmentVariable("RUSTUP_DIST_SERVER", "https://rsproxy.cn", "User")
        [Environment]::SetEnvironmentVariable("RUSTUP_UPDATE_ROOT", "https://rsproxy.cn/rustup", "User")
        ```

    - 2.2 Cấu hình gương phụ thuộc Cargo. Mở thư mục người dùng (`C:\Users\<username>\`), tìm hoặc tạo thư mục `.cargo`, sau đó tạo file `config.toml` bên trong.

        Dán nội dung sau vào `C:\Users\<username>\.cargo\config.toml`:

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

    - 2.3 Tải `rustup-init.exe` từ rustup.rs và chạy. Thực thi các lệnh sau trong PowerShell:

        ```powershell
        rustup toolchain install 1.94
        rustup default 1.94-x86_64-pc-windows-msvc
        ```

    - 2.4 Cài đặt wasm-bindgen-cli để tạo ràng buộc WASM-JS. Thực thi lệnh sau trong PowerShell:

        ```powershell
        cargo install -f wasm-bindgen-cli --version 0.2.117
        ```

## 3. Biên dịch

1. Tại gốc kho lưu trữ, mở PowerShell và thực thi:

    ```powershell
    cargo build --workspace --release
    ```

    > Bạn có thể gặp lỗi `error: failed to run custom build command for skia-bindings v0.97.2`
    
    - 1.1 Tải thủ công gói có backend GL: [skia-binaries-0.97.2 (GL)](https://github.com/rust-skia/skia-binaries/releases/download/0.97.2/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz)
    
    - 1.2 Đặt biến môi trường (thay bằng đường dẫn thực tế):

         ```powershell
        $env:SKIA_BINARIES_URL="file:///path/to/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz"
        ```
    
    - 1.3 Chạy lại bước 3.1 để biên dịch lại

2. Xây dựng Web Bundle

    - 2.1 Biên dịch thư viện WASM. Tại gốc kho lưu trữ, mở PowerShell và thực thi:

        ```powershell
        cargo build -p de-host-web --target wasm32-unknown-unknown --release --no-default-features --features "web canvaskit"
        ```

    - 2.2 Tạo file ràng buộc JS

        ```powershell
        wasm-bindgen target\wasm32-unknown-unknown\release\de_host_web.wasm --out-dir target\release\web-bundle --target web
        ```

    - 2.3 Sao chép tài nguyên CanvasKit rendering từ `crates\de-host-web\assets\canvaskit` vào `web-bundle`:

        ```powershell
        Copy-Item -Recurse "crates\de-host-web\assets\canvaskit" -Destination "target\release\web-bundle\canvaskit" -Force
        ```

    - 2.4 Cấu trúc thư mục cuối cùng

        ```plaintext
        target\release\
        ├── design-editor.exe
        ├── de-host-web-server.exe
        ├── de.exe
        └── web-bundle\
            ├── canvaskit\
            │   ├── canvaskit.js
            │   └── canvaskit.wasm
            ├── snippets\
            ├── op_host-web.d.ts
            ├── de_host_web.js
            ├── de_host_web_bg.wasm
            └── de_host_web_bg.wasm.d.ts
        ```

## 4. Cấu hình biến môi trường cho plugin dsh-design-editor

1. Chạy PowerShell với quyền quản trị viên và thực thi các lệnh sau (thay đường dẫn bằng gốc kho lưu trữ thực tế):

    ```powershell
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    ```

## 5. Xác minh

1. design-editor.exe

    ```powershell
    cd target\release
    ./design-editor.exe --serve-web 3100
    # Cửa sổ xuất hiện rồi đóng — dịch vụ chạy nền và lắng nghe cổng 3100
    ```

2. de-host-web-server.exe  <i>` (không bắt buộc cho plugin dsh) `</i>

    ```powershell
    cd target\release
    ./de-host-web-server.exe --serve-web 3100
    # Bạn sẽ thấy hai dòng "design-editor --serve-web:" và cổng 3100 trong PowerShell
    ```

---

# Khắc phục sự cố

1. Chú ý tham số `--features` khi biên dịch WASM

    ```powershell
    cargo build -p de-host-web --target wasm32-unknown-unknown --release --no-default-features --features "web canvaskit"
    ```

    > Sử dụng `--features "web canvaskit"` thay vì `--features "web"`.
    > 
    > `--features "web"` biên dịch thành công, nhưng file JS được tạo sẽ không xuất hàm `mount_ck`.
    > 
    >

2. Server ánh xạ toàn bộ thư mục `web-bundle` vào tuyến HTTP `/pkg/`

    > Đường dẫn ổ đĩa `web-bundle/de_host_web.js` → URL trình duyệt `/pkg/de_host_web.js`
    > 
    > Nếu trang hiển thị không tìm thấy `/pkg/de_host_web.js`, file bị thiếu trong `web-bundle/`.

3. Chuỗi công cụ rustup `1.94`, wasm-bindgen-cli `0.2.117` và skia-bindings `0.97.2` khớp với các phiên bản được cố định trong `rust-toolchain.toml` và `Cargo.lock`. Nếu các tệp đó ghi phiên bản khác, hãy dùng phiên bản đó.
