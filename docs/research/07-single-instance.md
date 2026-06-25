# Research 07 — Single-Instance Mutex

Verified facts for mutex check in `src/main.rs`.

---

## CreateMutexW Semantics

```rust
let handle = CreateMutexW(null(), FALSE, name.as_ptr());
let err = GetLastError();  // MUST call immediately — any subsequent Win32 call clears it

if handle.is_null() {
    // Real failure (access denied, OOM) — exit
    ExitProcess(1);
}
if err == ERROR_ALREADY_EXISTS {
    // Another instance running
    MessageBoxW(null_mut(), msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
    ExitProcess(0);
}
// We hold the mutex — proceed with init
```

- `CreateMutexW` returns a valid non-null handle even when `ERROR_ALREADY_EXISTS`
- `GetLastError` must be called BEFORE any other Win32 function
- `ERROR_ALREADY_EXISTS = 0xB7 = 183`
- Null return = real failure (different error code)
- Handle from `ERROR_ALREADY_EXISTS` path: `ExitProcess(0)` reclaims it — no explicit CloseHandle needed

## Global\ Namespace

- `"Global\\WraithSingleInstance"` — visible across ALL Terminal Services sessions
- Without prefix: each user session gets its own namespace (mutex not shared)
- Standard (non-elevated) users CAN create mutex in `Global\` namespace — allowed for sync objects
- Works across UAC elevation boundaries within the same session
- `HKLM`, `HKCU` etc. are irrelevant here — Global\ is the kernel object namespace

## MessageBoxW Before Window

- `MessageBoxW(null_mut(), ...)` valid before `RegisterClassExW` / `CreateWindowExW`
- Creates its own internal window — no parent required
- `MB_OK | MB_ICONINFORMATION` — in `Win32_UI_WindowsAndMessaging` feature

## ExitProcess

- `ExitProcess(0)` in `Win32_System_Threading` feature
- More immediate than returning from `main` (skips Rust runtime cleanup)
- Correct for the already-running case — no hooks installed, no resources to free yet
