# CASPER Shot

A tiny Windows screenshot tool: two global hotkeys, WebP output, a tray icon. No window, no dependency beyond `windows-sys`.

## Features

- Global hotkey to capture the monitor under your cursor (default `PrintScreen`).
- Global hotkey to drag-select a region, or click a window to snap to it, across any monitor (default `Shift+PrintScreen`) — both hotkeys are configurable, see below.
- Left-click the tray icon to jump straight into region-select mode, no hotkey needed.
- Every capture briefly previews at the bottom of the screen, then saves as `.webp`, sorted into a `YYYY-MM` subfolder. Left-click the preview to open the image, right-click it to open its containing folder.
- Tray icon menu (right-click): enable/disable the hotkeys, open the screenshots folder, Settings, Reload, GitHub, quit.

## Settings (`state.json`)

Created next to the executable on first run. Tray menu → Settings opens it (in Notepad if no app is associated with `.json`); Reload applies your changes. If the file contains invalid JSON, CASPER Shot uses the defaults and leaves the file untouched so you can fix it.

```json
{
  "quality": 90,
  "hotkey_fullscreen": "PrintScreen",
  "hotkey_region": "Shift+PrintScreen",
  "save_path": "%USERPROFILE%\\Pictures\\Screenshots",
  "preview_percent": 40,
  "preview_duration_ms": 2500
}
```

- `quality`: 0-99 is lossy WebP, 100 is lossless.
- `hotkey_*`: any combination of `Ctrl`/`Alt`/`Shift`/`Win` plus a letter, digit, F1-F12, or `PrintScreen`.
- `save_path`: supports `%ENV_VAR%` expansion; relative paths resolve next to the executable.
- `preview_percent`: width of the post-capture preview, as % of screen width; `0` disables the preview entirely.
- `preview_duration_ms`: how long the preview stays on screen before closing.

## Hotkeys not working?

`RegisterHotKey` only lets one process own a given key combination system-wide. If another running app already has the same combination bound (ShareX, Greenshot, Windows' own screen-snipping shortcut in Settings → Accessibility → Keyboard, etc.), CASPER Shot's registration fails silently and the tray tooltip reads "hotkeys unavailable" instead of listing the shortcuts. Close or reconfigure whichever app is holding them, or pick different shortcuts in `state.json` and Reload.
