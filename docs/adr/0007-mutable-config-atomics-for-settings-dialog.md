# Mutable config via AtomicU32, INI write-back for settings dialog

Supersedes the "config is read-only at runtime, no write-back" characterization
in the project brief. Issue #12 (Win32 settings dialog) and #13 (live hotkey
recorder field) require changing hotkey combos and the panic key at runtime
without a restart, and persisting those changes to `wraith.ini`.

## Decision

`Config`'s five runtime-tunable fields — `lock_mods`, `lock_vk`, `unlock_mods`,
`unlock_vk`, `panic_vk` — change from plain `u32` to `AtomicU32`. `lock_on_start`
becomes an `AtomicBool` for the same reason `write_back()` needs it: the
settings dialog's checkbox has to be able to change the persisted value even
though — unlike the other 5 fields — the in-memory value has no live effect
until the next restart (it's read exactly once, in `main.rs`, at startup).
Without interior mutability here, `write_back()` would have no way to persist
a checkbox change; it would just re-write whatever was loaded at startup.

The `Config` struct itself, and its `OnceLock<Config>` wrapper, are unchanged —
`Config::get()` still returns one `&'static Config` constructed once. Only the
fields inside it become independently mutable. This keeps `decide_action`
(hooks.rs) taking `&Config` as a plain parameter — still pure, still testable
without touching global state — callers just add `.load(Relaxed)` at read
sites.

The settings dialog's OK handler stores new values into these atomics directly
(`.store(v, Relaxed)`), taking effect on the very next keydown — `keyboard_proc`
already re-reads `Config::get()` fresh on every event (hooks.rs:160), so no
hook reinstall is needed. It then writes all 6 INI keys unconditionally via
`WritePrivateProfileStringW`, which updates matching keys in place and
preserves everything else in the file.

## Why not a Mutex, why not swap the whole Config

ADR 0003 established atomics-only, no-blocking-calls in the hook path. A
`Mutex<Config>` would violate that the moment the dialog holds the lock while
the hook tries to read it. Swapping the entire `&'static Config` behind
something like `ArcSwap` would work but adds a dependency and a layer of
indirection for a problem five `AtomicU32`s solve directly.

## Consequences

- A keydown that lands mid-write may read a partially-updated combo (e.g. new
  `lock_vk` with the old `lock_mods` for one event). Self-corrects on the next
  keystroke. Accepted — cheaper than synchronizing the 5 fields as one unit,
  and the failure mode is "one dropped/misfired combo attempt," not a crash
  or stuck lock state.
- The settings dialog must reject a lock/unlock combo with zero modifiers
  before writing it: `decide_action`'s modifier check (`held_mods & cfg.lock_mods
  == cfg.lock_mods`) is vacuously true when `lock_mods == 0`, so an
  unconstrained bare-key combo would fire on every press of that key
  system-wide. The panic key is exempt — it's intentionally single-key,
  checked via a separate hold-timer mechanism, not `decide_action`.
