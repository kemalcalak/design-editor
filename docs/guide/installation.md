# Installation

## macOS (Apple Silicon)

1. Download `Design-Editor-<version>-arm64-mac.dmg` from the [latest release](https://github.com/kemalcalak/design-editor/releases/latest).
2. Open it and drag **Design Editor** to Applications.
3. The app is not notarized yet, so macOS blocks the first launch: right-click the app and choose **Open**, or allow it under **System Settings → Privacy & Security**.

The `de` command-line tool ships inside the app. Put it on your `PATH`:

```bash
sudo ln -sf "/Applications/Design Editor.app/Contents/MacOS/de" /usr/local/bin/de
```

## Windows (x64)

An installer is on the way ([#35](https://github.com/kemalcalak/design-editor/issues/35)). Until then, [build from source](./building.md).

## Updates

The app looks for new releases on GitHub at startup and offers the installer. Turn this off under **Settings → System**.

Next: [connect an AI agent](./connect-agents.md).
