
> [English](./BUILD_WINDOWS.md) · [简体中文](./BUILD_WINDOWS.zh.md) · [繁體中文](./BUILD_WINDOWS.zh-TW.md) · [日本語](./BUILD_WINDOWS.ja.md) · [한국어](./BUILD_WINDOWS.ko.md) · [Français](./BUILD_WINDOWS.fr.md) · **Español** · [Deutsch](./BUILD_WINDOWS.de.md) · [Português](./BUILD_WINDOWS.pt.md) · [Русский](./BUILD_WINDOWS.ru.md) · [हिन्दी](./BUILD_WINDOWS.hi.md) · [Türkçe](./BUILD_WINDOWS.tr.md) · [ไทย](./BUILD_WINDOWS.th.md) · [Tiếng Việt](./BUILD_WINDOWS.vi.md) · [Bahasa Indonesia](./BUILD_WINDOWS.id.md)

# Guía de compilación de Design Editor para Windows

## 1. Descargar el código fuente

1. Descargue el código fuente del repositorio design-editor

2. design-editor tiene 3 submódulos de dependencias en el directorio `vendor` (jian, casement, agent) que deben descargarse por separado

    - 2.1 jian (framework UI): [https://github.com/ZSeven-W/jian](https://github.com/ZSeven-W/jian) → extraer en `vendor\jian`

    - 2.2 casement (wrapper de ventana): [https://github.com/ZSeven-W/casement](https://github.com/ZSeven-W/casement) → extraer en `vendor\casement`

    - 2.3 agent (runtime de Agent): [https://github.com/ZSeven-W/agent-rs](https://github.com/ZSeven-W/agent-rs) → extraer en `vendor\agent`

3. Después de extraer, verifique que existan las siguientes rutas:

    ```plaintext
    vendor\jian\Cargo.toml
    vendor\casement\Cargo.toml
    vendor\agent\Cargo.toml
    ```

## 2. Preparación del entorno — *Windows 10 x64 (cadena de herramientas MSVC)*

1. Instale Visual Studio seleccionando **"Desarrollo de escritorio con C++"**

2. Instale la cadena de herramientas Rust `stable-x86_64-pc-windows-msvc`

    - 2.1 Configure el espejo de rustup (para acelerar las descargas)

        ```powershell
        [Environment]::SetEnvironmentVariable("RUSTUP_DIST_SERVER", "https://rsproxy.cn", "User")
        [Environment]::SetEnvironmentVariable("RUSTUP_UPDATE_ROOT", "https://rsproxy.cn/rustup", "User")
        ```

    - 2.2 Configure el espejo de dependencias de Cargo. Abra su directorio de usuario (`C:\Users\<username>\`), encuentre o cree la carpeta `.cargo` y cree un archivo `config.toml` dentro.

        Pegue el siguiente contenido en `C:\Users\<username>\.cargo\config.toml`:

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

    - 2.3 Descargue `rustup-init.exe` desde rustup.rs y ejecútelo. En PowerShell ejecute los siguientes comandos:

        ```powershell
        rustup toolchain install 1.94
        rustup default 1.94-x86_64-pc-windows-msvc
        ```

## 3. Compilación

1. En la raíz del repositorio, abra PowerShell y ejecute:

    ```powershell
    cargo build --workspace --release
    ```

    > Puede encontrar el error `error: failed to run custom build command for skia-bindings v0.97.2`
    
    - 1.1 Descargue manualmente el paquete con backend GL: [skia-binaries-0.97.2 (GL)](https://github.com/rust-skia/skia-binaries/releases/download/0.97.2/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz)
    
    - 1.2 Establezca la variable de entorno (reemplace con su ruta real):

         ```powershell
        $env:SKIA_BINARIES_URL="file:///path/to/skia-binaries-da8fc6731fc439bc3b6a-x86_64-pc-windows-msvc-gl-jpegd-jpege-pdf-textlayout.tar.gz"
        ```
    
    - 1.3 Vuelva a ejecutar el paso 3.1 para recompilar

## 4. Configurar variables de entorno para el plugin dsh-design-editor

1. Ejecute PowerShell como administrador y ejecute los siguientes comandos (reemplace la ruta con la raíz real del repositorio):

    ```powershell
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    [Environment]::SetEnvironmentVariable("DSH_DESIGN_EDITOR_EDITOR_BINARY", "<design-editor-root>\target\release\design-editor.exe", "User")
    ```

## 5. Verificación

1. design-editor.exe

    ```powershell
    cd target\release
    ./design-editor.exe --mcp-http 3100 demo.de
    # La ventana aparece brevemente y se cierra — el servicio escucha en el puerto 3100 en segundo plano
    ```

---

# Solución de problemas

1. La cadena de herramientas rustup `1.94` y skia-bindings `0.97.2` corresponden a las versiones fijadas en `rust-toolchain.toml` y `Cargo.lock`. Si esos archivos indican otra versión, use la suya.
