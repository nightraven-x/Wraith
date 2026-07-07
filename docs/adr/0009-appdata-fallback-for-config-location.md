# %LOCALAPPDATA% fallback when no portable wraith.ini exists

Wraith always resolved `wraith.ini` relative to the exe (`GetModuleFileNameW`).
That's fine for a portable exe+ini folder, but the project's own installer
plan places Wraith in Program Files, and the UAC manifest requests
`asInvoker` (no elevation) — a standard user account cannot write to Program
Files. Once installed there, the settings dialog's changes would take effect
immediately in memory but silently fail to persist across a restart, since
`write_back()` would be trying to write into a directory the process has no
permission to write to.

## Decision

`Config::load()`/`write_back()` now resolve through `ini_path()`: if a
`wraith.ini` already exists next to the exe, use it (portable mode,
unchanged behavior for anyone carrying an exe+ini folder around). Otherwise,
resolve to `%LOCALAPPDATA%\Wraith\wraith.ini`, creating the `Wraith`
subdirectory if needed. `%LOCALAPPDATA%` is always writable by the owning
user without elevation, regardless of where the exe itself lives.

## Why hybrid, not AppData-only

Pure AppData-only was considered and rejected: it would silently relocate
config for anyone currently relying on a portable install (exe + ini in one
folder, no fixed install location) the moment they upgrade, with no
indication anything moved. Checking the exe-relative candidate first costs
one `GetFileAttributesW` call and preserves that use case exactly, while
still fixing the Program-Files case (nothing ships a portable ini there by
default, so it falls straight to AppData).

## Consequences

- `ini_path()` is dynamic, not cached — it re-checks portable-file existence
  on every call. This is deliberate (an install could plausibly go from
  "no portable ini" to "portable ini present" if a user manually drops one
  in later), but it also means tests that delete the portable candidate to
  exercise the AppData branch must restore it before releasing shared test
  state, or "portable missing" bleeds into whichever test runs next — see
  `config.rs`'s `RestorePortableIniOnDrop` and `lock_config_test()`, the
  single entry point all config/settings tests now use to acquire the
  shared test lock specifically so this invariant holds regardless of test
  order.
- No migration path for an existing installed user's Program-Files
  `wraith.ini` (if one somehow got created there, e.g. by manually copying
  the dev-built exe+ini together into Program Files) — it would keep being
  read (still exists, still wins as "portable") but any *new* write attempt
  there would still fail the same way it always did; this ADR only fixes
  the case where no such file exists yet.
