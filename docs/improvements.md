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
- clear_captures pending-composer ordering — already resolved: history.rs closes the composer
  window and clears CaptureState.captures before remove_dir_all.
- M4 deferred: cmd.exe has no profile mechanism — native Tier 1 covers
  PowerShell only; a tray "New Claude Code session" launcher would cover cmd
  users (rejected for now: only Taco-started sessions would be injectable).
- M4 upgrade note: `wsl_connected` defaults to false — existing WSL-shim users
  must reinstall the WSL shim (or the flag flips on their next install) before
  WSL sessions reappear in the composer.
- deployer assumes `$USERPROFILE\Documents` — redirected Documents folders
  break profile edits (ponytail comment in deployer.rs).
- M4 deferred: no opportunistic cleanup of stale native `run\*.json` (crashed
  shims) — stale entries surface in the list and fall to Tier 2 on send, same
  as stale WSL entries today; add a connect-probe sweep if the noise bothers.
- M4 known: Windows can't unlink a still-bound AF_UNIX socket file, so a clean
  shim exit leaves its `.sock` behind (the `.json` registry entry IS removed;
  the pre-bind remove_file handles staleness on next start). Restructure the
  accept loop for drop-before-cleanup if the litter ever matters.
- M4 known: with ExecutionPolicy Restricted/AllSigned the PowerShell profile
  never runs and the 'claude' wrapper is inert — install surfaces a warning
  (deployer::exec_policy_warning) but cannot fix it for the user.
- M4 risk: an npm-installed native Claude Code is `claude.cmd`, which the
  shim's bare-name CreateProcess spawn may not resolve; verified against the
  native installer's claude.exe only. If it bites, resolve the full path in
  the profile function via `(Get-Command claude -CommandType Application).Source`.
- `remove_native_shim` reports "removed" even when the exe was locked by a
  live shim session (`std::fs::remove_file(...).ok()` swallows the error) —
  surface the leftover like install's mirror copy does instead of ignoring it.
- `native_bin_dir` falls back to `.` when LOCALAPPDATA is unset while
  `native_run_dir` returns `None` for the same case — the two helpers
  disagree on an unreachable-on-Windows case.
- `wsl_scan_is_gated` asserts wall-clock <200ms — right tell, could flake on
  a loaded machine; a spawn-counting seam would be sturdier.
- `profile_paths` guesses `$USERPROFILE\Documents` — could instead ask
  powershell for `$PROFILE.CurrentUserAllHosts` (deployer already shells
  powershell for Get-ExecutionPolicy).
