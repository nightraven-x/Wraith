# Spec 07 — Single-Instance Mutex

> Research prerequisite: [../research/07-single-instance.md](../research/07-single-instance.md)
> Implements: `src/main.rs` (top of main) — Step 1

---

## Purpose

Prevent more than one Wraith process from running simultaneously. Uses a named
Win32 mutex with a `Global\` prefix so it works across user sessions and UAC
elevation boundaries.

---

## Behavioral Requirements

**R1.** Call `CreateMutexW(NULL, FALSE, L"Global\\WraithSingleInstance")` at the
very start of `main`, before any other initialization.

**R2.** Check `GetLastError()` immediately after `CreateMutexW` (even on success —
`CreateMutexW` returns a handle AND sets `ERROR_ALREADY_EXISTS` if the mutex
already existed).

**R3.** If `GetLastError() == ERROR_ALREADY_EXISTS`:
- Call `MessageBoxW(NULL, L"Wraith is already running.", L"Wraith", MB_OK | MB_ICONINFORMATION)`.
- Call `ExitProcess(0)` (not panic — clean exit, not an error condition).

**R4.** If `CreateMutexW` returns NULL (not just ERROR_ALREADY_EXISTS but a real
creation failure): call `MessageBoxW` with a different message ("Failed to create
mutex.") and `ExitProcess(1)`.

**R5.** On success: store the handle. Do NOT close it until process exit — closing
the handle releases the mutex, allowing a second instance to start.

**R6.** The handle need not be explicitly closed at exit — the OS closes all handles
when the process terminates.

---

## Mutex Name

`"Global\\WraithSingleInstance"` — the `Global\` prefix makes it visible across
all sessions (Terminal Services, fast user switching). Without it, each user session
could run its own instance.

---

## MessageBox Before Window Exists

The `MessageBoxW` call in R3 happens before `RegisterClassExW` and `CreateWindowExW`.
This is valid — `MessageBoxW` with `hWnd = NULL` creates its own top-level window
internally. No parent window needed.

---

## Dependencies

- Nothing. This is the first thing in `main`.

---

## Edge Cases

- **Elevated vs non-elevated instances:** A `Global\` mutex is shared across integrity
  levels in the same session. Verify in research whether this holds across UAC
  elevation on the same machine.
- **ERROR_ACCESS_DENIED on Global\ namespace:** On some locked-down systems, creating
  objects in the `Global\` namespace requires elevated privileges. If this occurs,
  fall back to a session-local name `"WraithSingleInstance"` (without `Global\`).
  Document this as a known limitation. Verify in research.
- **Handle leak if ExitProcess called:** ExitProcess terminates without running
  destructors. The OS reclaims the handle. Acceptable.
