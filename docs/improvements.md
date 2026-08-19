# Improvement list

Deferred work, in rough priority order. Each lands as its own small branch when picked up.

## From user testing (2026-08-19)

- ~~Hotkey reassignment in Settings~~ — shipped on m3-p1-extras (rebindable region/window/clipboard
  hotkeys in Settings, live re-registration, fallback to defaults on invalid/taken bindings).
  Known limitation: tray menu accelerator labels don't update after a remap until restart.
- ~~Composer too small for annotation~~ — shipped on m3-p1-extras (resizable composer,
  min 420×380, preview/canvas fill the window, size remembered like position).

## Deferred minors from M3 reviews (all cosmetic/observability; none block anything)

- "no foreground window" error also covers DWM failures (misleading wording)
- `window_sc`/`clip_sc` shortcuts rebuilt on every hotkey callback (negligible)
- `read_image` failure always reported as "No image on the clipboard"
- `ManagerExt` imported inline twice in lib.rs; is_enabled check-then-act race (single-instance app)
- History: `.PNG` (uppercase) files excluded by case-sensitive filter
- History/delete errors conflate traversal-guard rejection vs file-already-gone
- Annotation: click-without-drag pushes an invisible 1-point shape → needless re-save on send
- Annotation: no way to exit annotate mode back to plain preview (Esc cancels whole capture)
- Preview image lacks `cursor: pointer` affordance for "click to annotate"
- `settings::save` uses `.expect` on serialization (unfailable types today)
- `retention_hours: 0` accepted if settings.json is hand-edited (UI clamps to ≥1)
- `Settings` struct lacks `Debug` derive
