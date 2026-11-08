# ChatGameOverlay

[🇩🇪 Deutsch](README.md) · 🇬🇧 English

Shows a channel's Twitch chat as a **transparent, always-on-top window** – right over the game you're playing. Written in Rust, available as an installer or a single `.exe` that needs no installation, no Twitch login required.

## Features

- Read any channel's chat live (anonymously, no login or token)
- Borderless, transparent, always-on-top window
- **Lock mode:** mouse clicks go through the overlay into the game
- Global hotkeys that work while the game has focus
- Twitch name colors, badges for broadcaster, mods, VIPs and subs, `/me` messages in italics
- Deleted messages, timeouts and bans also disappear from the overlay
- Adjustable font size, background opacity and automatic fade-out of old messages
- Settings in their own regular window – the chat window shows the chosen transparency live
- Tray icon with menu; settings also reachable via the taskbar
- Window position, size and settings are saved
- Installer with Start menu entry, optional desktop shortcut and autostart
- Built-in updater
- German or English interface (follows the Windows language on first start, can be changed in the settings)

## Installation

[Releases](https://github.com/areadox/ChatGameOverlay/releases) offer two variants:

- **`ChatGameOverlay-Setup.exe`** – installer (recommended). Installs without admin rights to `%LOCALAPPDATA%\Programs\ChatGameOverlay`, creates a Start menu entry and optionally a desktop shortcut and autostart with Windows. Uninstall via *Settings → Apps*.
- **`ChatGameOverlay-windows.exe`** – single program file without installation, e.g. for a USB stick.

The files are not digitally signed, so Windows SmartScreen warns on first start: *More info → Run anyway*.

## Usage

1. Start ChatGameOverlay.
2. On first start the settings window opens: enter the channel name (e.g. `areadox`) and click **Connect**.
3. Drag the chat window by its top bar to where you want it, resize it with the grip in the bottom-right corner. Background opacity and font size can be changed in the settings window meanwhile – the chat window shows the effect immediately.
4. Click **🔒** or press **Ctrl + Shift + F9** – the overlay is now locked and no longer catches clicks.

| Hotkey | Action |
|---|---|
| Ctrl + Shift + F9 | Lock ⇄ edit (move, settings) |
| Ctrl + Shift + F10 | Show / hide overlay |

### Language

Use the **Deutsch / English** switch at the top of the settings window. The program, tray menu and update messages change immediately.

### Taskbar and tray

The chat window itself does not appear in the taskbar. Instead, **ChatGameOverlay** in the taskbar stands for the settings window: clicking it opens the settings, and the settings window's **✖** puts it back into the taskbar without quitting the program.

The tray icon sits in the notification area next to the clock:

- **Left click:** open settings
- **Right click:** menu with *Open settings*, *Locked*, *Hidden* and *Quit*

Quit the program via the tray menu, the **Quit program** button in the settings, or the **✖** in the chat window's top bar.

### Important: the game's display mode

The game must run in **borderless window** (windowed fullscreen) mode. In **exclusive fullscreen** the game draws directly to the screen and Windows won't show any other window on top of it. Most games have this setting under Graphics → Display mode.

### Note for streaming

If OBS captures the game with **Game Capture** or **Window Capture**, the overlay is not visible in the stream – only you see it. With **Display Capture** it is captured too.

## Updates

On start the program checks GitHub for a newer version (can be turned off in the settings). If there is one, a green **⬆** appears at the top of the window. In the settings under **Updates**, click **Update now** – the new version is downloaded and replaces the program file, and **Restart now** (or **⟳** at the top) starts it right away. Settings and window position are kept.

This requires write access to the program's own folder. With the installer that's automatic; don't put the single `.exe` under `C:\Program Files`, use e.g. your user folder.

## Where settings are stored

- Windows: `%APPDATA%\ChatGameOverlay\`
- Linux: `~/.local/share/ChatGameOverlay/`

## Building

```bash
cargo build --release
```

The program file ends up in `target/release/`.

The installer is built with [Inno Setup](https://jrsoftware.org/isinfo.php) from `installer.iss` (expects the program file as `dist\ChatGameOverlay.exe`):

```bash
iscc /DMyAppVersion=0.0.5 installer.iss
```

GitHub Actions builds a Windows and a Linux version on every push (downloadable under "Actions").

### Creating a release

Releases are only created on explicit request, in one of these ways:

- **By commit:** bump the version in `Cargo.toml` and push a commit to `main` whose first line is `Release vX.Y.Z` (matching the version). The remaining lines of the commit message become the release text. The workflow builds, creates the tag and attaches the files.
- **By tag:** create a new tag such as `v0.0.5` in the browser under *Releases → Draft a new release*, or push it with `git push origin v0.0.5`.

## Known limitations

- Emotes are shown as text (e.g. `Kappa`), not as images.
- The hotkeys can't be changed yet.
- Version 0.0.1 has no updater – switch to a newer version manually once, after that it's automatic.
- On Linux, always-on-top and click-through depend on the window manager; there is no tray icon there yet.

The full changelog (in German) is in the [German README](README.md#änderungen).
