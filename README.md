# ChatGameOverlay

🇩🇪 Deutsch · [🇬🇧 English](README.en.md)

Zeigt den Twitch-Chat eines Kanals als **durchsichtiges Fenster immer im Vordergrund** – direkt über dem Spiel, das gerade läuft. Geschrieben in Rust, mit Installer oder als einzelne `.exe` ohne Installation, kein Twitch-Login nötig.

## Funktionen

- Chat eines beliebigen Kanals live mitlesen – anonym ohne Login, oder angemeldet mit deinem Twitch-Konto
- **Mit Twitch anmelden** und direkt aus dem Overlay schreiben, auch per Hotkey mitten im Spiel
- Kopieren und Einfügen mit Strg+C/V und Rechtsklick-Menü; Rechtsklick auf Nachrichten zum Kopieren
- **Kanalpunkte:** Einlösungen deiner Zuschauer und „Nachricht hervorheben“ erscheinen farbig hervorgehoben im Chat
- Randloses, transparentes Fenster, immer im Vordergrund
- **Fixier-Modus:** Mausklicks gehen durch das Overlay ins Spiel
- Globale Hotkeys, die auch funktionieren, während das Spiel den Fokus hat
- Namensfarben wie bei Twitch, `/me`-Nachrichten kursiv
- **Twitch-Emotes als Bild** direkt im Text
- **Echte Twitch-Abzeichen** (Streamer, Mod, VIP, Abo-Stufen des Kanals, Prime …) – mit Anmeldung; ohne Anmeldung einfache Symbole
- Gelöschte Nachrichten, Timeouts und Banns verschwinden auch im Overlay
- Einstellbar: Schriftgröße, Deckkraft des Hintergrunds, automatisches Ausblenden alter Nachrichten
- Einstellungen in einem eigenen, normalen Fenster – das Chatfenster zeigt dabei live die eingestellte Transparenz
- Symbol im Infobereich (Tray) mit Menü; Einstellungen auch über die Taskleiste erreichbar
- Fensterposition, Größe und Einstellungen werden gespeichert
- Installer mit Startmenü-Eintrag, optionaler Desktop-Verknüpfung und Autostart
- Eingebaute Update-Funktion
- Oberfläche auf Deutsch oder Englisch (beim ersten Start nach Windows-Sprache, umschaltbar in den Einstellungen)

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
| Strg + Umschalt + F11 | Im Chat schreiben (nur angemeldet) |

### Mit Twitch anmelden und schreiben

Zum Mitlesen ist keine Anmeldung nötig. Wer auch schreiben möchte:

1. In den Einstellungen unter **Twitch-Konto** auf **Mit Twitch anmelden** klicken.
2. Der Browser öffnet `twitch.tv/activate` mit einem Code, der auch im Einstellungsfenster steht. Dort bei Twitch anmelden (falls noch nicht) und auf **Autorisieren** klicken.
3. Fertig – das Einstellungsfenster zeigt „Angemeldet als …“.

Das Passwort wird dabei nie im Programm eingegeben; das Programm bekommt von Twitch nur die Berechtigung, den Chat zu lesen und zu schreiben (`chat:read`, `chat:edit`) und die Kanalpunkte-Einlösungen in deinem Kanal zu sehen (`channel:read:redemptions`). Die Anmeldung bleibt gespeichert und wird automatisch erneuert. **Abmelden** in den Einstellungen nimmt die Berechtigung bei Twitch auch wieder zurück.

**Schreiben:**

- Im Bearbeiten-Modus gibt es unten im Chatfenster eine Eingabezeile. **Enter** sendet.
- Mitten im Spiel: **Strg + Umschalt + F11** – das Overlay holt sich die Tastatur, Nachricht tippen, **Enter** sendet, und der Fokus geht automatisch ans Spiel zurück. **Esc** bricht ab.
- `/me Text` wird als Aktion gesendet. Andere Chat-Befehle (`/ban`, `/timeout` …) werden nicht unterstützt.

### Kanalpunkte

Löst ein Zuschauer eine Kanalpunkte-Belohnung ein, erscheint das als farbiger Kasten im Chat, z. B. *◆ Max hat „Trink was!“ eingelöst · 500* – bei Belohnungen mit Texteingabe samt Text. Automatische Belohnungen (Emote freischalten) werden ebenso angezeigt, und Nachrichten mit **„Nachricht hervorheben“** bekommen denselben farbigen Rahmen.

In den Einstellungen unter **Kanalpunkte**: Einlösungen und Hervorhebungen einzeln an- und abschalten, die **Farbe** wählen und mit **Vorschau** Beispiele ins Chatfenster holen.

- **Alle** Einlösungen gibt Twitch nur für den **eigenen Kanal** heraus – also mit deinem Twitch-Konto angemeldet und deinen Kanal eingestellt. Der Status in den Einstellungen zeigt dann „✔ Einlösungen werden empfangen“.
- Ohne Anmeldung oder in fremden Kanälen erscheinen nur Belohnungen mit Texteingabe (die stehen ohnehin im Chat) und „Nachricht hervorheben“.
- Wer sich mit Version 0.0.7 angemeldet hat, muss sich **einmal neu anmelden**, weil die Kanalpunkte eine zusätzliche Berechtigung brauchen. Das Einstellungsfenster bietet dafür einen Knopf **Neu anmelden**.

### Kopieren und Einfügen

In allen Eingabefeldern funktionieren **Strg + C / X / V / A** und ein **Rechtsklick-Menü** mit *Ausschneiden*, *Kopieren*, *Einfügen* und *Alles auswählen*. Ein Rechtsklick auf eine Chatnachricht (im Bearbeiten-Modus) bietet *Nachricht kopieren*, *Name kopieren* und *Name und Nachricht kopieren*.

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

- Windows: `%APPDATA%\chat-game-overlay\data\app.ron`
- Linux: `~/.local/share/chat-game-overlay/app.ron`

Dort liegt auch die Twitch-Anmeldung (Zugangs-Token). Wer sich abmeldet, entfernt sie wieder.

## Selbst bauen

```bash
cargo build --release
```

Die fertige Datei liegt dann unter `target/release/`.

Für die Twitch-Anmeldung braucht das Programm die Client-ID einer eigenen Twitch-Anwendung ([dev.twitch.tv/console](https://dev.twitch.tv/console), Typ **Public**, Redirect-URL `http://localhost`). Sie steht in `src/auth.rs` (`CLIENT_ID`); ist sie leer, ist die Anmeldung abgeschaltet und das Programm liest nur mit.

Der Installer wird mit [Inno Setup](https://jrsoftware.org/isinfo.php) aus `installer.iss` gebaut (erwartet die Programmdatei als `dist\ChatGameOverlay.exe`):

```bash
iscc /DMyAppVersion=0.0.3 installer.iss
```

GitHub Actions baut bei jedem Push automatisch eine Windows- und eine Linux-Version (unter „Actions“ zum Download).

### Release erstellen

Ein Release entsteht nur auf ausdrücklichen Wunsch, auf einem dieser Wege:

- **Per Commit:** Version in `Cargo.toml` hochzählen und einen Commit nach `main` pushen, dessen erste Zeile `Release vX.Y.Z` lautet (passend zur Version). Die weiteren Zeilen der Commit-Nachricht werden zum Release-Text. Der Workflow baut, legt den Tag an und hängt die Dateien an.
- **Per Tag:** im Browser unter *Releases → Draft a new release* einen neuen Tag wie `v0.0.3` anlegen oder ihn per `git push origin v0.0.3` hochladen.

## Bekannte Grenzen

- Emotes von BetterTTV, FrankerFaceZ und 7TV werden noch als Text angezeigt; animierte Twitch-Emotes erscheinen als Standbild.
- Die Hotkeys sind noch fest eingestellt.
- Version 0.0.1 hat noch keine Update-Funktion – einmal von Hand auf eine neuere Version wechseln, danach geht es automatisch.
- Unter Linux hängt „immer im Vordergrund“ und das Durchklicken vom Fenstermanager ab; ein Tray-Symbol gibt es dort noch nicht.

## Änderungen

### Nächste Version
- Twitch-Emotes werden als Bild im Text angezeigt (auch breite Emotes im richtigen Seitenverhältnis). Emotes, die im Chat schon vorkamen oder zum Kanal gehören, erscheinen auch in eigenen Nachrichten und Kanalpunkte-Texten als Bild.
- Echte Twitch-Abzeichen statt Symbolen (mit Anmeldung), inklusive der Abo-Abzeichen des Kanals.
- Neue Einstellung „Emotes als Bild anzeigen“; die Vorschau zeigt jetzt auch Abzeichen und Emotes.

### 0.0.9
- Beim Update über „Jetzt neu starten“ wird das alte Programm jetzt zuverlässig beendet; die neue Version wartet, bis es zu ist. (Beim Update von 0.0.8 auf 0.0.9 das alte Programm noch einmal selbst beenden, z. B. über das Tray-Menü.)

### 0.0.8
- **Kanalpunkte:** Einlösungen deiner Zuschauer erscheinen hervorgehoben im Chat (eigene und automatische Belohnungen, mit Titel, Kosten und Text); „Nachricht hervorheben“ wird farbig markiert. Farbe wählbar, Vorschau in den Einstellungen.
- Neue Berechtigung `channel:read:redemptions` – nach dem Update einmal neu anmelden.

### 0.0.7
- Twitch-Anmeldung freigeschaltet: über „Mit Twitch anmelden“ als du selbst anmelden und im Chat schreiben.

### 0.0.6
- **Twitch-Anmeldung** (über twitch.tv/activate, ohne Passworteingabe im Programm) und Schreiben im Chat – über die Eingabezeile im Chatfenster oder per **Strg + Umschalt + F11** mitten im Spiel. Vorbereitet; wird freigeschaltet, sobald die Twitch-Anwendung registriert ist.
- Hinweise von Twitch (z. B. Slow-Mode) erscheinen im Chat.
- Kopieren und Einfügen funktionieren wieder (Strg+C/V), und Links lassen sich öffnen – beides war seit 0.0.2 versehentlich abgeschaltet.
- Rechtsklick-Menü für Eingabefelder (Ausschneiden, Kopieren, Einfügen, Alles auswählen) und für Chatnachrichten (Nachricht/Name kopieren).
- Speicherort der Einstellungen in der README korrigiert.

### 0.0.5
- Sprachauswahl Deutsch / Englisch für Programm, Tray-Menü und Update-Meldungen; beim ersten Start wird die Windows-Sprache übernommen.
- Installer auf Deutsch und Englisch.
- README auf Englisch.

### 0.0.4
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
