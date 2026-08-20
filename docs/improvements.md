# Improvement list

Deferred work, in rough priority order. Each lands as its own small branch when picked up.

## From user testing (2026-08-19)

- ~~Hotkey reassignment in Settings~~ — shipped on m3-p1-extras (rebindable region/window/clipboard
  hotkeys in Settings, live re-registration, fallback to defaults on invalid/taken bindings).
  Tray menu accelerator labels update after a remap (fixed on m3-p1-extras Task 10).
- ~~Composer too small for annotation~~ — shipped on m3-p1-extras (resizable composer,
  min 420×380, preview/canvas fill the window, size remembered like position).

## Deferred minors from M3 reviews (all cosmetic/observability; none block anything)

- ~~"no foreground window" error also covers DWM failures (misleading wording)~~ — fixed:
  `save_active_window` now returns "could not read the active window bounds".
- ~~`window_sc`/`clip_sc` shortcuts rebuilt on every hotkey callback (negligible)~~ — already
  resolved: lib.rs dispatch reads the managed `Hotkeys` state (no per-callback construction).
- ~~`read_image` failure always reported as "No image on the clipboard"~~ — fixed:
  `save_clipboard_image` now includes the underlying error in the message.
- ~~`ManagerExt` imported inline twice in lib.rs~~ — fixed: hoisted to one file-top import.
  `is_enabled` check-then-act race — wontfix (single-instance app, no realistic concurrent writer).
- ~~History: `.PNG` (uppercase) files excluded by case-sensitive filter~~ — fixed: `list_under`
  now compares extensions case-insensitively.
- ~~History/delete errors conflate traversal-guard rejection vs file-already-gone~~ — fixed:
  `resend_capture`/`delete_capture` return "file no longer exists" when the path is gone.
- ~~Annotation: click-without-drag pushes an invisible 1-point shape → needless re-save on send~~ —
  fixed: `up()` pops the just-drawn shape when it has fewer than 2 points.
- ~~Annotation: no way to exit annotate mode back to plain preview (Esc cancels whole capture)~~ —
  fixed: a "Done" button applies the annotation (composites + saves) and exits annotate mode.
- ~~Preview image lacks `cursor: pointer` affordance for "click to annotate"~~ — already resolved:
  `.preview` in Composer.vue already sets `cursor: pointer`.
- ~~`settings::save` uses `.expect` on serialization (unfailable types today)~~ — fixed: propagates
  serialize errors via `map_err(std::io::Error::other)` instead of panicking.
- ~~`retention_hours: 0` accepted if settings.json is hand-edited (UI clamps to ≥1)~~ — fixed:
  `settings::load` now clamps `retention_hours` to a floor of 1 server-side.
- ~~`Settings` struct lacks `Debug` derive~~ — fixed: `Debug` added to the derive list.
