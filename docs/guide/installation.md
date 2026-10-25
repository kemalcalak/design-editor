# Installation

## macOS (Apple Silicon)

1. Download `Design-Editor-<version>-arm64-mac.dmg` from the [latest release](https://github.com/kemalcalak/design-editor/releases/latest).
2. Open it and drag **Design Editor** to Applications.
3. The app is not notarized yet, so macOS blocks the first launch: right-click the app and choose **Open**, or allow it under **System Settings → Privacy & Security**.

The `de` command-line tool ships inside the app. Put it on your `PATH`:

```bash
sudo ln -sf "/Applications/Design Editor.app/Contents/MacOS/de" /usr/local/bin/de
```

It is also available on its own as [`de-cli-macos-aarch64.tar.gz`](https://github.com/kemalcalak/design-editor/releases/latest/download/de-cli-macos-aarch64.tar.gz).

## Windows (x64)

1. Download `Design-Editor-<version>-x64-win-setup.exe` from the [latest release](https://github.com/kemalcalak/design-editor/releases/latest).
2. Run it. The installer is not signed yet, so SmartScreen may warn on the first run: choose **More info → Run anyway**.

The `de.exe` command-line tool is installed next to the app in `C:\Program Files\Design Editor` but is not added to `PATH`. It is also available on its own as [`de-cli-windows-x86_64.zip`](https://github.com/kemalcalak/design-editor/releases/latest/download/de-cli-windows-x86_64.zip).

## Linux (x86_64)

1. Download [`design-editor-linux-x86_64.tar.gz`](https://github.com/kemalcalak/design-editor/releases/latest/download/design-editor-linux-x86_64.tar.gz) from the latest release. It holds the bare `design-editor` binary, with no installer or package.
2. Unpack it into a folder you can write to, for example:

   ```bash
   tar xzf design-editor-linux-x86_64.tar.gz -C ~/.local/bin
   ```

3. Run `design-editor`.

The build needs glibc 2.39 or newer (Ubuntu 24.04 or a distribution of the same age), GL/EGL, and X11 or Wayland.

The standalone `de` command-line tool is in [`de-cli-linux-x86_64.tar.gz`](https://github.com/kemalcalak/design-editor/releases/latest/download/de-cli-linux-x86_64.tar.gz), also a bare binary:

```bash
tar xzf de-cli-linux-x86_64.tar.gz -C ~/.local/bin
```

## Updates

The app looks for new releases on GitHub at startup and offers the installer. On Linux it unpacks the new binary from the same tarball and replaces the running one, so keep it in a folder you can write to. Turn this off under **Settings → System**.

Next: [connect an AI agent](./connect-agents.md).
