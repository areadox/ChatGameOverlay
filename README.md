# ChatGameOverlay

Zeigt den Twitch-Chat eines Kanals als **durchsichtiges Fenster immer im Vordergrund** – direkt über dem Spiel, das gerade läuft. Geschrieben in Rust, eine einzelne `.exe`, keine Installation, kein Twitch-Login nötig.

## Funktionen

- Chat eines beliebigen Kanals live mitlesen (anonym, ohne Login oder Token)
- Randloses, transparentes Fenster, immer im Vordergrund
- **Fixier-Modus:** Mausklicks gehen durch das Overlay ins Spiel
- Globale Hotkeys, die auch funktionieren, während das Spiel den Fokus hat
- Namensfarben wie bei Twitch, Abzeichen für Streamer, Mods, VIPs und Subs, `/me`-Nachrichten kursiv
- Gelöschte Nachrichten, Timeouts und Banns verschwinden auch im Overlay
- Einstellbar: Schriftgröße, Deckkraft des Hintergrunds, automatisches Ausblenden alter Nachrichten
- Fensterposition, Größe und Einstellungen werden gespeichert

## Bedienung

1. `ChatGameOverlay-windows.exe` starten.
2. Beim ersten Start öffnen sich die Einstellungen: Kanalnamen eintragen (z. B. `areadox`) und **Verbinden** klicken.
3. Fenster am Titel (**✥ #kanal**) an die gewünschte Stelle ziehen, Größe über den Griff unten rechts ändern.
4. Auf **🔒** klicken oder **Strg + Umschalt + F9** drücken – das Overlay ist jetzt fixiert und klickt nicht mehr dazwischen.

| Hotkey | Wirkung |
|---|---|
| Strg + Umschalt + F9 | Fixieren ⇄ Bearbeiten (verschieben, Einstellungen) |
| Strg + Umschalt + F10 | Overlay ein- / ausblenden |

### Wichtig: Anzeigemodus des Spiels

Das Spiel muss als **„Randloses Fenster“** (Borderless / Fenster-Vollbild) laufen. Im **exklusiven Vollbild** zeichnet das Spiel direkt auf den Bildschirm, und Windows lässt dann kein anderes Fenster darüber erscheinen. Bei den meisten Spielen findet sich die Einstellung unter Grafik → Anzeigemodus.

### Hinweis für den Stream

Wird das Spiel in OBS per **Spielaufnahme** oder **Fensteraufnahme** erfasst, ist das Overlay im Stream nicht zu sehen – nur du siehst es. Bei **Bildschirmaufnahme** wird es mit aufgenommen.

## Updates

Beim Start prüft das Programm, ob es auf GitHub eine neuere Version gibt (abschaltbar in den Einstellungen). Gibt es eine, erscheint oben im Fenster ein grüner **⬆**. In den Einstellungen unter **Updates** auf **Jetzt aktualisieren** klicken – die neue Version wird heruntergeladen, ersetzt die Programmdatei, und mit **Jetzt neu starten** (oder **⟳** oben) läuft sie sofort. Einstellungen und Fensterposition bleiben erhalten.

Dafür muss das Programm in seinem eigenen Ordner Schreibrechte haben – also z. B. nicht unter `C:\Programme` ablegen, sondern etwa im Benutzerordner.

## Speicherort der Einstellungen

- Windows: `%APPDATA%\ChatGameOverlay\`
- Linux: `~/.local/share/ChatGameOverlay/`

## Selbst bauen

```bash
cargo build --release
```

Die fertige Datei liegt dann unter `target/release/`.

GitHub Actions baut bei jedem Push automatisch eine Windows- und eine Linux-Version (unter „Actions“ zum Download). Ein Release wird erst erstellt, wenn ein Versions-Tag gepusht wird:

```bash
git tag v0.0.1
git push origin v0.0.1
```

## Bekannte Grenzen (v0.0.1)

- Emotes werden als Text angezeigt (z. B. `Kappa`), nicht als Bild.
- Die Hotkeys sind noch fest eingestellt.
- Version 0.0.1 hat noch keine Update-Funktion – einmal von Hand auf 0.0.2 wechseln, danach geht es automatisch.
- Unter Linux hängt „immer im Vordergrund“ und das Durchklicken vom Fenstermanager ab.

## Änderungen

### 0.0.2
- Update-Funktion: prüft beim Start auf neue Versionen und aktualisiert sich auf Knopfdruck selbst.
- Fenster ist unter Windows jetzt wirklich durchsichtig: Darstellung über OpenGL statt DirectX/Vulkan, da diese unter Windows keine transparenten Fenster unterstützen.

### 0.0.1
- Erste Version.
