# Improvement list

Deferred work, in rough priority order. Each lands as its own small branch when picked up.

## From user testing (2026-08-19)

- **Hotkey reassignment in Settings** — three fields (region / active window / clipboard)
  stored in `settings.json`, registered at startup instead of the hard-coded shortcuts,
  re-registered live on save; invalid/conflicting bindings rejected via notification,
  old binding kept. (Product spec §20 lists this; cut from M3 for minimalism.)
- **Composer too small for annotation** — window is fixed 420×380 with the preview capped
  at 160px, so editing the capture is cramped. Fix: make the composer resizable
  (`.resizable(true)`, sensible min size), let the preview/canvas flex to fill available
  height instead of the 160px cap, and remember size alongside the remembered position.

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
