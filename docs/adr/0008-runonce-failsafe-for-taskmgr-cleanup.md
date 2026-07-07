# RunOnce failsafe for a stuck DisableTaskMgr policy

A security audit of the full codebase found two related issues around
`app.rs`'s Task Manager anti-circumvention feature (`task_mgr_block()`/
`task_mgr_unblock()`, toggling `HKCU\...\Policies\System\DisableTaskMgr` while
locked — closes the one gap `Ctrl+Alt+Del`'s unblockable secure-desktop screen
leaves, since its own Task Manager can kill wraith.exe and fully escape the
lock):

1. `WM_DESTROY` never called `task_mgr_unblock()`. Clicking "Exit" from the
   tray menu while locked — no crash needed, fully deterministic — left the
   policy set system-wide with no process left running to clear it.
2. Even with (1) fixed, a forced kill (`TerminateProcess`, e.g. via a remote
   session), a crash, or power loss gives a dying process no chance to run
   any cleanup code at all. This is a hard OS-level limitation, not something
   fixable from inside the process.

## Decision

(1) is fixed directly: `WM_DESTROY` now calls `task_mgr_unblock()`
unconditionally, not gated on `LOCKED`.

(2) is mitigated with an `HKCU\...\RunOnce` entry, registered by
`task_mgr_block()` and removed by `task_mgr_unblock()` on any clean path. The
entry re-invokes `wraith.exe --cleanup-taskmgr` — a fast-path in `main.rs`,
checked before the single-instance mutex, that just calls
`app::startup_cleanup()` (already-existing, already clears the policy) and
exits immediately. Windows guarantees `RunOnce` entries fire at the next
interactive logon regardless of how the previous session ended, which is
exactly the crash-safety property needed.

## Why not remove the feature entirely

Considered removing `task_mgr_block()`/`unblock()` outright — physical input
is already blocked while locked, so in the common case Task Manager's UI is
unreachable anyway. Rejected: `Ctrl+Alt+Del` is kernel-hardwired and cannot be
blocked (see the top-level project brief's Hard Limits), and its
secure-desktop screen offers its own Task Manager, completely outside
Wraith's hook. Without `DisableTaskMgr`, someone at the desk could press
Ctrl+Alt+Del, open Task Manager from there, and kill wraith.exe — fully
circumventing the lock. The feature closes a real gap; removing it reopens
it.

## Why not a companion process or service

A watchdog process/service that clears the policy if Wraith dies would be
more robust (real crash-time cleanup, not just next-logon), but conflicts
with the project's single-.exe, no-async-runtime, minimal-dependency
constraints (see `CLAUDE.md`). `RunOnce` achieves the same crash-safety
property (cleanup survives even if Wraith never runs again) using only a
registry write the codebase already makes routinely, with no new process,
service, or dependency.

## Consequences

- Between an unclean termination and the next interactive logon, Task
  Manager stays disabled system-wide for that user. Bounded to "until next
  logon" instead of "forever" — a real improvement, not a full fix.
- `main.rs`'s `--cleanup-taskmgr` invocation is internal-only (not
  documented as a user-facing flag) and deliberately skips the entire normal
  startup sequence (mutex, hooks, tray) — it must run even when a real
  Wraith instance is already up, or when none is.
- Registry writes to `Policies\System` are not universally reliable — at
  least one dev machine actively denies them (`ERROR_ACCESS_DENIED`), likely
  via GPO/hardening targeting this exact key. `task_mgr_block()` correctly
  no-ops in that case, and no RunOnce failsafe gets registered either (there
  is nothing to fail-safe against if the block itself never took effect).
  This also means the anti-circumvention protection itself may already be
  neutralized on well-managed/hardened machines — acceptable, since Wraith
  can't do anything about a host-level ACL, and the failure mode is "no
  protection", not "broken lock" or "false unlock".
