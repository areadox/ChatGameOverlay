# ChatGameOverlay

Zeigt den Twitch-Chat eines Kanals als **durchsichtiges Fenster immer im Vordergrund** – direkt über dem Spiel, das gerade läuft. Geschrieben in Rust, mit Installer oder als einzelne `.exe` ohne Installation, kein Twitch-Login nötig.

## Funktionen

- Chat eines beliebigen Kanals live mitlesen (anonym, ohne Login oder Token)
- Randloses, transparentes Fenster, immer im Vordergrund
- **Fixier-Modus:** Mausklicks gehen durch das Overlay ins Spiel
- Globale Hotkeys, die auch funktionieren, während das Spiel den Fokus hat
- Namensfarben wie bei Twitch, Abzeichen für Streamer, Mods, VIPs und Subs, `/me`-Nachrichten kursiv
- Gelöschte Nachrichten, Timeouts und Banns verschwinden auch im Overlay
- Einstellbar: Schriftgröße, Deckkraft des Hintergrunds, automatisches Ausblenden alter Nachrichten
- Einstellungen in einem eigenen, normalen Fenster – das Chatfenster zeigt dabei live die eingestellte Transparenz
- Symbol im Infobereich (Tray) mit Menü; Einstellungen auch über die Taskleiste erreichbar
- Fensterposition, Größe und Einstellungen werden gespeichert
- Installer mit Startmenü-Eintrag, optionaler Desktop-Verknüpfung und Autostart
- Eingebaute Update-Funktion

## Installation

Unter [Releases](https://github.com/areadox/ChatGameOverlay/releases) gibt es zwei Varianten:

- **`ChatGameOverlay-Setup.exe`** – Installer (empfohlen). Installiert ohne Admin-Rechte nach `%LOCALAPPDATA%\Programs\ChatGameOverlay`, legt einen Startmenü-Eintrag an und auf Wunsch eine Desktop-Verknüpfung und den Autostart mit Windows. Deinstallieren über *Einstellungen → Apps*.
- **`ChatGameOverlay-windows.exe`** – einzelne Programmdatei ohne Installation, z. B. für einen USB-Stick.

Da die Dateien nicht digital signiert sind, warnt Windows SmartScreen beim ersten Start: *Weitere Informationen → Trotzdem ausführen*.

## Bedienung

1. ChatGameOverlay starten.
2. Beim ersten Start öffnet sich das Einstellungsfenster: Kanalnamen eintragen (z. B. `areadox`) und **Verbinden** klicken.
3. Das Chatfenster an der oberen Leiste an die gewünschte Stelle ziehen, Größe über den Griff unten rechts ändern. Hintergrund-Deckkraft und Schriftgröße lassen sich währenddessen im Einstellungsfenster ändern – das Chatfenster zeigt die Wirkung sofort.
4. Auf **🔒** klicken oder **Strg + Umschalt + F9** drücken – das Overlay ist jetzt fixiert und klickt nicht mehr dazwischen.

| Hotkey | Wirkung |
|---|---|
| Strg + Umschalt + F9 | Fixieren ⇄ Bearbeiten (verschieben, Einstellungen) |
| Strg + Umschalt + F10 | Overlay ein- / ausblenden |

### Taskleiste und Tray

Das Chatfenster selbst erscheint nicht in der Taskleiste. Dort steht stattdessen **ChatGameOverlay** für das Einstellungsfenster: ein Klick darauf öffnet die Einstellungen, das **✖** des Einstellungsfensters legt es wieder in die Taskleiste zurück, ohne das Programm zu beenden.

Im Infobereich neben der Uhr sitzt das Tray-Symbol:

- **Linksklick:** Einstellungen öffnen
- **Rechtsklick:** Menü mit *Einstellungen öffnen*, *Fixiert*, *Ausgeblendet* und *Beenden*

Beenden lässt sich das Programm über das Tray-Menü, den Knopf **Programm beenden** in den Einstellungen oder das **✖** in der oberen Leiste des Chatfensters.

### Wichtig: Anzeigemodus des Spiels

Das Spiel muss als **„Randloses Fenster“** (Borderless / Fenster-Vollbild) laufen. Im **exklusiven Vollbild** zeichnet das Spiel direkt auf den Bildschirm, und Windows lässt dann kein anderes Fenster darüber erscheinen. Bei den meisten Spielen findet sich die Einstellung unter Grafik → Anzeigemodus.

### Hinweis für den Stream

Wird das Spiel in OBS per **Spielaufnahme** oder **Fensteraufnahme** erfasst, ist das Overlay im Stream nicht zu sehen – nur du siehst es. Bei **Bildschirmaufnahme** wird es mit aufgenommen.

## Updates

Beim Start prüft das Programm, ob es auf GitHub eine neuere Version gibt (abschaltbar in den Einstellungen). Gibt es eine, erscheint oben im Fenster ein grüner **⬆**. In den Einstellungen unter **Updates** auf **Jetzt aktualisieren** klicken – die neue Version wird heruntergeladen, ersetzt die Programmdatei, und mit **Jetzt neu starten** (oder **⟳** oben) läuft sie sofort. Einstellungen und Fensterposition bleiben erhalten.

Dafür muss das Programm in seinem eigenen Ordner Schreibrechte haben. Beim Installer ist das automatisch so; die einzelne `.exe` also nicht unter `C:\Programme` ablegen, sondern etwa im Benutzerordner.

## Speicherort der Einstellungen

- Windows: `%APPDATA%\ChatGameOverlay\`
- Linux: `~/.local/share/ChatGameOverlay/`

## Selbst bauen

```bash
cargo build --release
```

Die fertige Datei liegt dann unter `target/release/`.

Der Installer wird mit [Inno Setup](https://jrsoftware.org/isinfo.php) aus `installer.iss` gebaut (erwartet die Programmdatei als `dist\ChatGameOverlay.exe`):

```bash
iscc /DMyAppVersion=0.0.3 installer.iss
```

GitHub Actions baut bei jedem Push automatisch eine Windows- und eine Linux-Version (unter „Actions“ zum Download).

### Release erstellen

Ein Release entsteht nur auf ausdrücklichen Wunsch, auf einem dieser Wege:

- **Per Commit:** Version in `Cargo.toml` hochzählen und einen Commit nach `main` pushen, dessen erste Zeile `Release vX.Y.Z` lautet (passend zur Version). Die weiteren Zeilen der Commit-Nachricht werden zum Release-Text. Der Workflow baut, legt den Tag an und hängt die Dateien an.
- **Per Tag:** im Browser unter *Releases → Draft a new release* einen neuen Tag wie `v0.0.3` anlegen oder ihn per `git push origin v0.0.3` hochladen.

## Bekannte Grenzen (v0.0.1)

- Emotes werden als Text angezeigt (z. B. `Kappa`), nicht als Bild.
- Die Hotkeys sind noch fest eingestellt.
- Version 0.0.1 hat noch keine Update-Funktion – einmal von Hand auf 0.0.2 wechseln, danach geht es automatisch.
- Unter Linux hängt „immer im Vordergrund“ und das Durchklicken vom Fenstermanager ab; ein Tray-Symbol gibt es dort noch nicht.

## Änderungen

### Nächste Version
- Tray-Symbol mit Menü (Einstellungen, Fixiert, Ausgeblendet, Beenden); Linksklick öffnet die Einstellungen.
- Einstellungsfenster dauerhaft in der Taskleiste, Schließen legt es dort ab; das Chatfenster erscheint nicht mehr in der Taskleiste.
- Knopf „Programm beenden“ in den Einstellungen.

### 0.0.3
- Das Chatfenster lässt sich an der ganzen oberen Leiste verschieben; die Leiste ist nicht mehr durchsichtig.
- Beim Bearbeiten zeigt das Chatfenster genau die eingestellte Transparenz.
- Einstellungen in einem eigenen, nicht durchsichtigen Fenster.
- Installer (`ChatGameOverlay-Setup.exe`) mit Startmenü, Desktop-Verknüpfung und Autostart.
- Programmsymbol in der `.exe`.

### 0.0.2
- Update-Funktion: prüft beim Start auf neue Versionen und aktualisiert sich auf Knopfdruck selbst.
- Fenster ist unter Windows jetzt wirklich durchsichtig: Darstellung über OpenGL statt DirectX/Vulkan, da diese unter Windows keine transparenten Fenster unterstützen.

### 0.0.1
- Erste Version.
