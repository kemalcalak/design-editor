
> [English](./BUILD_WINDOWS.md) · [简体中文](./BUILD_WINDOWS.zh.md) · [繁體中文](./BUILD_WINDOWS.zh-TW.md) · [日本語](./BUILD_WINDOWS.ja.md) · [한국어](./BUILD_WINDOWS.ko.md) · [Français](./BUILD_WINDOWS.fr.md) · [Español](./BUILD_WINDOWS.es.md) · [Deutsch](./BUILD_WINDOWS.de.md) · [Português](./BUILD_WINDOWS.pt.md) · [Русский](./BUILD_WINDOWS.ru.md) · [हिन्दी](./BUILD_WINDOWS.hi.md) · [Türkçe](./BUILD_WINDOWS.tr.md) · **ไทย** · [Tiếng Việt](./BUILD_WINDOWS.vi.md) · [Bahasa Indonesia](./BUILD_WINDOWS.id.md)

# คู่มือการคอมไพล์ Design Editor สำหรับ Windows

## 1. ดาวน์โหลดซอร์สโค้ด

1. ดาวน์โหลดซอร์สโค้ดจากคลังเก็บ design-editor

2. design-editor มีโมดูลย่อย 3 ตัวในไดเรกทอรี `vendor` (jian, casement, agent) ที่ต้องดาวน์โหลดแยกต่างหาก

    - 2.1 jian (เฟรมเวิร์ก UI): [https://github.com/ZSeven-W/jian](https://github.com/ZSeven-W/jian) → แตกไฟล์ไปยัง `vendor\jian`

    - 2.2 casement (ตัวห่อหน้าต่าง): [https://github.com/ZSeven-W/casement](https://github.com/ZSeven-W/casement) → แตกไฟล์ไปยัง `vendor\casement`

    - 2.3 agent (รันไทม์ Agent): [https://github.com/ZSeven-W/agent-rs](https://github.com/ZSeven-W/agent-rs) → แตกไฟล์ไปยัง `vendor\agent`

3. หลังแตกไฟล์ ตรวจสอบว่าเส้นทางต่อไปนี้มีอยู่:

    ```plaintext
    vendor\jian\Cargo.toml
    vendor\casement\Cargo.toml
    vendor\agent\Cargo.toml
    ```

## 2. การเตรียมสภาพแวดล้อม — *Windows 10 x64 (MSVC toolchain)*

1. ติดตั้ง Visual Studio โดยเลือก **"การพัฒนาเดสก์ท็อปด้วย C++"**

2. ติดตั้ง toolchain Rust `stable-x86_64-pc-windows-msvc`

    - 2.1 กำหนดค่ามิเรอร์ rustup (เพื่อเร่งความเร็วในการดาวน์โหลด)

        ```powershell
        [Environment]::SetEnvironmentVariable("RUSTUP_DIST_SERVER", "https://rsproxy.cn", "User")
        [Environment]::SetEnvironmentVariable("RUSTUP_UPDATE_ROOT", "https://rsproxy.cn/rustup", "User")
        ```

    - 2.2 กำหนดค่ามิเรอร์依赖ของ Cargo เปิดไดเรกทอรีผู้ใช้ (`C:\Users\<username>\`) ค้นหาหรือสร้างโฟลเดอร์ `.cargo` แล้วสร้างไฟล์ `config.toml` ภายใน

        วางเนื้อหาต่อไปนี้ใน `C:\Users\<username>\.cargo\config.toml`:

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

    - 2.3 ดาวน์โหลด `rustup-init.exe` จาก rustup.rs แล้วรัน เป็นคำสั่งใน PowerShell:

        ```powershell
        rustup toolchain install 1.94
        rustup default 1.94-x86_64-pc-windows-msvc
        ```

## 3. การคอมไพล์

1. ในรูทของคลังเก็บ เปิด PowerShell แล้วรัน:

    ```powershell
    cargo build --workspace --release
    ```

    > คุณอาจพบข้อผิดพลาด `error: failed to run custom build command for skia-bindings v0.97.2`
    
    - 1.1 ดาวน์โหลดแพ็กเกจ GL backend ด้วยตนเอง: [skia-binaries-0.97.2 (GL)](https://github.com/rust-skia/skia-binaries/releases/download/0.97.2/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz)
    
    - 1.2 ตั้งค่า environment variable (แทนที่ด้วยเส้นทางจริง):

         ```powershell
        $env:SKIA_BINARIES_URL="file:///path/to/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz"
        ```
    
    - 1.3 รันขั้นตอน 3.1 อีกครั้งเพื่อคอมไพล์ใหม่

## 4. กำหนดค่า environment variable สำหรับปลั๊กอิน dsh-design-editor

1. เปิด PowerShell ในฐานะผู้ดูแลระบบแล้วรันคำสั่งต่อไปนี้ (แทนที่เส้นทางด้วยรูทคลังเก็บจริง):

    ```powershell
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    ```

## 5. การตรวจสอบ

1. design-editor.exe

    ```powershell
    cd target\release
    ./design-editor.exe --mcp-http 3100 demo.de
    # หน้าต่างจะกะพริบแล้วปิด — บริการทำงานในพื้นหลังและฟังพอร์ต 3100
    ```

---

# การแก้ไขปัญหา

1. rustup toolchain `1.94` และ skia-bindings `0.97.2` ตรงกับเวอร์ชันที่กำหนดไว้ใน `rust-toolchain.toml` และ `Cargo.lock` หากไฟล์เหล่านั้นระบุเวอร์ชันอื่น ให้ใช้เวอร์ชันนั้น
