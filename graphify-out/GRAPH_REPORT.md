# Graph Report - .  (2026-07-07)

## Corpus Check
- Corpus is ~30,530 words - fits in a single context window. You may not need a graph.

## Summary
- 369 nodes · 445 edges · 38 communities detected
- Extraction: 86% EXTRACTED · 13% INFERRED · 0% AMBIGUOUS · INFERRED: 60 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Architecture Decision Records|Architecture Decision Records]]
- [[_COMMUNITY_Win32 API Primitives|Win32 API Primitives]]
- [[_COMMUNITY_Hook Callback Internals|Hook Callback Internals]]
- [[_COMMUNITY_TrayLock State Management|Tray/Lock State Management]]
- [[_COMMUNITY_Hook InstallUninstall Atomics|Hook Install/Uninstall Atomics]]
- [[_COMMUNITY_Domain Glossary Terms|Domain Glossary Terms]]
- [[_COMMUNITY_PRD Core Requirements|PRD Core Requirements]]
- [[_COMMUNITY_Feature Requirements List|Feature Requirements List]]
- [[_COMMUNITY_Tray Icon Implementation|Tray Icon Implementation]]
- [[_COMMUNITY_Update Checker Implementation|Update Checker Implementation]]
- [[_COMMUNITY_Hook Callback Helpers|Hook Callback Helpers]]
- [[_COMMUNITY_README User Docs|README User Docs]]
- [[_COMMUNITY_CI Build Workflow|CI Build Workflow]]
- [[_COMMUNITY_Tray Icon Assets (SVG)|Tray Icon Assets (SVG)]]
- [[_COMMUNITY_App Lifecycle Functions|App Lifecycle Functions]]
- [[_COMMUNITY_Autostart Registry Details|Autostart Registry Details]]
- [[_COMMUNITY_Message-Only Window Limitations|Message-Only Window Limitations]]
- [[_COMMUNITY_NSIS Installer CI|NSIS Installer CI]]
- [[_COMMUNITY_Config Module|Config Module]]
- [[_COMMUNITY_Issue Tracker Conventions|Issue Tracker Conventions]]
- [[_COMMUNITY_Autostart Module|Autostart Module]]
- [[_COMMUNITY_Injected-Flag Hook Structs|Injected-Flag Hook Structs]]
- [[_COMMUNITY_Single-Instance Mutex|Single-Instance Mutex]]
- [[_COMMUNITY_Domain Doc Rules|Domain Doc Rules]]
- [[_COMMUNITY_main.rs Entry Point|main.rs Entry Point]]
- [[_COMMUNITY_Config INI Parsing|Config INI Parsing]]
- [[_COMMUNITY_Resource Embedding (windres)|Resource Embedding (windres)]]
- [[_COMMUNITY_Tray Icon DPILoading|Tray Icon DPI/Loading]]
- [[_COMMUNITY_build.rs Entry|build.rs Entry]]
- [[_COMMUNITY_Hook nCode Handling|Hook nCode Handling]]
- [[_COMMUNITY_Autostart Registry Cleanup|Autostart Registry Cleanup]]
- [[_COMMUNITY_Updater Version Parsing|Updater Version Parsing]]
- [[_COMMUNITY_Manifest Resource Embedding|Manifest Resource Embedding]]
- [[_COMMUNITY_Message Pump Detail|Message Pump Detail]]
- [[_COMMUNITY_Tray Balloon Notification|Tray Balloon Notification]]
- [[_COMMUNITY_Autostart Constants|Autostart Constants]]
- [[_COMMUNITY_Tray Balloon Method|Tray Balloon Method]]
- [[_COMMUNITY_WndProc Message Constants|WndProc Message Constants]]

## God Nodes (most connected - your core abstractions)
1. `Contributing Guide Project Overview` - 18 edges
2. `Wraith Domain Glossary` - 17 edges
3. `PRD Solution Summary` - 16 edges
4. `Specification Index (SPEC.md)` - 14 edges
5. `Wraith Claude Code Project Brief` - 12 edges
6. `v1.0.0 Initial Release` - 9 edges
7. `TrayIcon` - 8 edges
8. `Wraith Project Overview` - 8 edges
9. `Module Layout` - 8 edges
10. `Module Boundaries (Implementation Decisions)` - 8 edges

## Surprising Connections (you probably didn't know these)
- `PRD Problem Statement` --semantically_similar_to--> `Why Not BlockInput Rationale`  [INFERRED] [semantically similar]
  docs/PRD.md → README.md
- `/grill-with-docs Skill` --semantically_similar_to--> `Contributing Guidelines Section`  [INFERRED] [semantically similar]
  docs/SKILLS.md → CONTRIBUTING.md
- `asInvoker Execution Level (No UAC)` --semantically_similar_to--> `Known Limitations Section`  [INFERRED] [semantically similar]
  SECURITY.md → CONTRIBUTING.md
- `Out of Scope Section` --semantically_similar_to--> `Ctrl+Alt+Del Out of Scope`  [INFERRED] [semantically similar]
  docs/PRD.md → SECURITY.md
- `SetThreadExecutionState Invariant (PRD)` --conceptually_related_to--> `Key Constraints`  [INFERRED]
  docs/PRD.md → CLAUDE.md

## Hyperedges (group relationships)
- **Hook Callbacks Must Never Block (Atomics-Only Pattern)** — contributing_global_state, adr0003_decision, prd_global_state_hook_callbacks, claude_key_constraints [INFERRED 0.85]
- **Injected-Flag-First Hook Decision Pattern** — readme_injected_flag_mechanism, context_injected_flag, prd_injected_flag_ordering, claude_architecture_data_flow [INFERRED 0.80]
- **PostMessageW-Only Hook-to-App Communication Pattern** — adr0004_decision, prd_combo_detection, contributing_module_hooks, claude_what_not_to_do [INFERRED 0.85]
- **Cross-Thread PostMessageW-Only Communication Pattern** — pump_hook_pump_requirement, updater_postmessagew_from_thread, lock_wndproc_handlers [INFERRED 0.85]
- **HWND_MESSAGE Broadcast-Message Blind Spot (WM_ENDSESSION/TaskbarCreated)** — pump_wm_endsession_not_received, pump_taskbarcreated_not_received, tray_taskbarcreated_limitation, lock_taskbarcreated_recovery [EXTRACTED 0.90]
- **Build-Artifact-Installer-Release CI Pipeline** — ci_full_build_yml, ci_download_artifact, installer_nsis_on_gha [EXTRACTED 0.85]
- **Startup Init Sequence (mutex -> config -> window -> tray -> hooks -> lock_on_start -> updater)** — 02msgpump_init_sequence, 07single_mutex_check, 03config_load, 04tray_new, 01hooks_install, 05lock_lock_fn, 09updater_spawn [EXTRACTED 0.90]
- **Lock/Unlock State Lifecycle (atomics, timer, tray icon)** — 05lock_lock_fn, 05lock_unlock_fn, 06panic_timer_setup, 01hooks_locked_atomic, 04tray_set_locked [EXTRACTED 0.85]
- **Release Build Pipeline (CI build -> resource embed -> NSIS installer)** — 12ci_build_job, 12ci_installer_job, 11installer_nsi_script, 10resources_buildrs [EXTRACTED 0.85]

## Communities

### Community 0 - "Architecture Decision Records"
Cohesion: 0.06
Nodes (52): ADR-0001: Rust over C++ and Go (Decision), ADR-0001 Rationale: GC Pauses / Memory Safety, ADR-0002: windows-sys over windows Crate (Decision), ADR-0002 Rationale: GNU Target Compatibility, ADR-0003: Global Atomics over Mutex (Decision), ADR-0003 Rationale: Hook Deadline Cannot Block, ADR-0004: PostMessageW Only From Hooks (Decision), ADR-0004 Rationale: SendMessageW Deadlock Risk (+44 more)

### Community 1 - "Win32 API Primitives"
Cohesion: 0.07
Nodes (28): SetWindowsHookExW Signature, Hook Thread Affinity, GWLP_USERDATA TrayIcon Pointer Pattern, SetThreadExecutionState Semantics, SetTimer / KillTimer for Panic Timer, WM_COMMAND Dispatch (LOWORD), WndProc Message Handler Table, GetAsyncKeyState (Panic Hold Detection) (+20 more)

### Community 2 - "Hook Callback Internals"
Cohesion: 0.09
Nodes (28): keyboard_proc, mouse_proc, Rationale: nCode < 0 mandatory short-circuit, PANIC_START atomic (hooks.rs), Hook Architecture Purpose, Rationale: LLKHF/LLMHF_INJECTED always pass through, Message Window + Pump Purpose, Config::get() (+20 more)

### Community 3 - "Tray/Lock State Management"
Cohesion: 0.1
Nodes (26): LOCKED atomic (AtomicBool), Shutdown (PostQuitMessage / WM_QUIT), TrayIcon::destroy(), TrayIcon::set_locked(), TrayIcon::show_menu(), Explorer Restart Recovery (TaskbarCreated), WM_TRAY_MSG Routing, app::lock() (+18 more)

### Community 4 - "Hook Install/Uninstall Atomics"
Cohesion: 0.09
Nodes (25): APP_HWND atomic, Edge case: hook removal on timeout, hooks::install(), KB_HOOK atomic, MOUSE_HOOK atomic, hooks::uninstall(), Rationale: HWND_MESSAGE window drives hook pump, Init Sequence (main.rs) (+17 more)

### Community 5 - "Domain Glossary Terms"
Cohesion: 0.1
Nodes (22): Config (Term), Example Dialogue, Hook (Term), Hook Pump (Term), Injected Flag (Term), Locked State (Term), Panic Unlock (Term), Physical Input (Term) (+14 more)

### Community 6 - "PRD Core Requirements"
Cohesion: 0.1
Nodes (21): Autostart Registry Implementation (PRD), Further Notes Section, HWND_MESSAGE Window (PRD), Module Boundaries (Implementation Decisions), PRD Problem Statement, SetThreadExecutionState Invariant (PRD), PRD Solution Summary, Testing Decisions Section (+13 more)

### Community 7 - "Feature Requirements List"
Cohesion: 0.12
Nodes (18): Autostart Toggle Feature, DisableTaskMgr Policy Feature, WH_KEYBOARD_LL + WH_MOUSE_LL Hooks Feature, Hook Watchdog (5s Reinstall) Feature, Configurable Lock/Unlock Hotkeys + Panic Unlock Feature, Modifier Key-Up Passthrough Feature, Single-Instance Enforcement Feature, System Tray Icon Feature (+10 more)

### Community 8 - "Tray Icon Implementation"
Cohesion: 0.32
Nodes (4): blank_nid(), copy_wide(), load_icons(), TrayIcon

### Community 9 - "Update Checker Implementation"
Cohesion: 0.25
Nodes (5): fetch_latest(), parse_tag(), parse_ver(), parse_ver_numeric_comparison_correct(), spawn()

### Community 10 - "Hook Callback Helpers"
Cohesion: 0.27
Nodes (7): install(), is_modifier_vk(), keyboard_proc(), mod_held(), mods_held(), uninstall(), watchdog()

### Community 11 - "README User Docs"
Cohesion: 0.18
Nodes (11): Building From Source Section, wraith.ini Configuration Section, Controls Table, Getting Started (Installer/Portable), Injected Flag Mechanism Explanation, GPL-3.0 License Section, Project Layout Section, Wraith Project Overview (+3 more)

### Community 12 - "CI Build Workflow"
Cohesion: 0.2
Nodes (11): dtolnay/rust-toolchain Action, Full Verified build.yml Structure, GITHUB_TOKEN contents:write Permission, MinGW Install on ubuntu-latest, softprops/action-gh-release@v2 Usage, Workflow Trigger Pattern (tags v*.*.*), Config Struct Defaults (wraith.ini), OnceLock for Config (+3 more)

### Community 13 - "Tray Icon Assets (SVG)"
Cohesion: 0.27
Nodes (10): Locked Tray Icon (locked.svg), Keyhole Cutout (circle + teardrop stem), Padlock Body (rounded rectangle, filled), Padlock Shackle (closed arc, stroke-only), Padlock Body with Keyhole (rounded rect + circle + teardrop slot), Unlocked Padlock Tray Icon, Open/Swung-Open Padlock Shackle (rotated -26deg arc), Wraith LOCKED Application State (hooks.rs LOCKED atomic) (+2 more)

### Community 14 - "App Lifecycle Functions"
Cohesion: 0.53
Nodes (8): lock(), startup_cleanup(), task_mgr_block(), task_mgr_unblock(), toggle(), tray(), unlock(), wnd_proc()

### Community 15 - "Autostart Registry Details"
Cohesion: 0.25
Nodes (8): Registry Access Rights (KEY_SET_VALUE/KEY_QUERY_VALUE), HKCU Run Key, Path Quoting Requirement, RegSetValueExW for REG_SZ, HKCU Autostart Cleanup Bug (Elevated Uninstaller), Start Menu Shortcuts, NSIS Uninstall Section, Uninstaller Registration (Add/Remove Programs)

### Community 16 - "Message-Only Window Limitations"
Cohesion: 0.33
Nodes (7): TaskbarCreated Recovery Not Implementable, No WM_ENDSESSION Handler (app.rs), HWND_MESSAGE Window Properties, TaskbarCreated Not Received on HWND_MESSAGE, WM_ENDSESSION Not Received on HWND_MESSAGE, WNDCLASSEXW Required Fields, Tray TaskbarCreated Limitation (No Recovery)

### Community 17 - "NSIS Installer CI"
Cohesion: 0.33
Nodes (6): actions/download-artifact@v4 Behavior, makensis on windows-latest, NSIS File Section (Install), NSIS Pre-Installed on windows-latest, NSIS Script Fundamentals, shell: cmd Not Required for makensis

### Community 18 - "Config Module"
Cohesion: 0.5
Nodes (2): Config, exe_relative()

### Community 19 - "Issue Tracker Conventions"
Cohesion: 0.4
Nodes (5): 'Fetch Relevant Ticket' Convention, gh CLI Issue/PR Conventions, PRs as Triage Surface, 'Publish to Issue Tracker' Convention, Triage Label Mapping Table

### Community 20 - "Autostart Module"
Cohesion: 0.5
Nodes (0): 

### Community 21 - "Injected-Flag Hook Structs"
Cohesion: 0.5
Nodes (4): Blocking Events (return nonzero, no CallNextHookEx), LLKHF_INJECTED Keyboard Flag, LLMHF_INJECTED Mouse Flag, KBDLLHOOKSTRUCT / MSLLHOOKSTRUCT Fields (windows-sys 0.59)

### Community 22 - "Single-Instance Mutex"
Cohesion: 0.5
Nodes (4): CreateMutexW Semantics, ExitProcess for Already-Running Case, Global\ Namespace for Mutex, MessageBoxW Before Window Creation

### Community 23 - "Domain Doc Rules"
Cohesion: 0.5
Nodes (4): ADR Conflict Flagging Rule, docs/adr/ Architecture Decision Records, CONTEXT.md Canonical Vocabulary, Glossary Vocabulary Usage Rule

### Community 24 - "main.rs Entry Point"
Cohesion: 1.0
Nodes (2): main(), to_wide()

### Community 25 - "Config INI Parsing"
Cohesion: 0.67
Nodes (3): GetModuleFileNameW Usage, GetPrivateProfileIntW Semantics, INI Path Construction

### Community 26 - "Resource Embedding (windres)"
Cohesion: 0.67
Nodes (3): Linking resource.o via build.rs, VERSIONINFO Resource Block, windres Cross-Compiler Invocation

### Community 27 - "Tray Icon DPI/Loading"
Cohesion: 0.67
Nodes (3): DPI Awareness for Tray Icon, Icon Resource ID and MAKEINTRESOURCEW, Icon Loading (LoadImageW / LoadIconW Fallback)

### Community 28 - "build.rs Entry"
Cohesion: 1.0
Nodes (0): 

### Community 29 - "Hook nCode Handling"
Cohesion: 1.0
Nodes (2): CallNextHookEx Usage, nCode Values (HC_ACTION / negative)

### Community 30 - "Autostart Registry Cleanup"
Cohesion: 1.0
Nodes (2): is_autostart() Implementation, RegDeleteValueW Behavior

### Community 31 - "Updater Version Parsing"
Cohesion: 1.0
Nodes (2): GitHub Releases API Endpoint, Version Parsing (No serde/json)

### Community 32 - "Manifest Resource Embedding"
Cohesion: 1.0
Nodes (2): RT_MANIFEST Resource Embedding, wraith.manifest (UAC + DPI)

### Community 33 - "Message Pump Detail"
Cohesion: 1.0
Nodes (1): TranslateMessage on Message-Only Window

### Community 34 - "Tray Balloon Notification"
Cohesion: 1.0
Nodes (1): Balloon Notification Fields

### Community 35 - "Autostart Constants"
Cohesion: 1.0
Nodes (1): Autostart windows-sys Constants

### Community 36 - "Tray Balloon Method"
Cohesion: 1.0
Nodes (0): 

### Community 37 - "WndProc Message Constants"
Cohesion: 1.0
Nodes (1): WM_/ID_ Constants (app.rs)

## Ambiguous Edges - Review These
- `Unlocked Padlock Tray Icon` → `Unlocked Padlock Tray Icon`  [AMBIGUOUS]
  assets/unlocked.svg · relation: semantically_similar_to

## Knowledge Gaps
- **130 isolated node(s):** `Vulnerability Reporting Process`, `Synthetic Input Passthrough (Known Security Behaviour)`, `WH_KEYBOARD_LL + WH_MOUSE_LL Hooks Feature`, `Configurable Lock/Unlock Hotkeys + Panic Unlock Feature`, `System Tray Icon Feature` (+125 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `build.rs Entry`** (2 nodes): `main()`, `build.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Hook nCode Handling`** (2 nodes): `CallNextHookEx Usage`, `nCode Values (HC_ACTION / negative)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Autostart Registry Cleanup`** (2 nodes): `is_autostart() Implementation`, `RegDeleteValueW Behavior`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Updater Version Parsing`** (2 nodes): `GitHub Releases API Endpoint`, `Version Parsing (No serde/json)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Manifest Resource Embedding`** (2 nodes): `RT_MANIFEST Resource Embedding`, `wraith.manifest (UAC + DPI)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Message Pump Detail`** (1 nodes): `TranslateMessage on Message-Only Window`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Tray Balloon Notification`** (1 nodes): `Balloon Notification Fields`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Autostart Constants`** (1 nodes): `Autostart windows-sys Constants`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Tray Balloon Method`** (1 nodes): `TrayIcon::show_balloon()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `WndProc Message Constants`** (1 nodes): `WM_/ID_ Constants (app.rs)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What is the exact relationship between `Unlocked Padlock Tray Icon` and `Unlocked Padlock Tray Icon`?**
  _Edge tagged AMBIGUOUS (relation: semantically_similar_to) - confidence is low._
- **Why does `Contributing Guide Project Overview` connect `Architecture Decision Records` to `README User Docs`, `Feature Requirements List`?**
  _High betweenness centrality (0.039) - this node is a cross-community bridge._
- **Why does `Wraith Claude Code Project Brief` connect `Architecture Decision Records` to `Domain Glossary Terms`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._
- **Why does `Specification Index (SPEC.md)` connect `Hook Callback Internals` to `Tray/Lock State Management`, `Hook Install/Uninstall Atomics`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **What connects `Vulnerability Reporting Process`, `Synthetic Input Passthrough (Known Security Behaviour)`, `WH_KEYBOARD_LL + WH_MOUSE_LL Hooks Feature` to the rest of the system?**
  _130 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Architecture Decision Records` be split into smaller, more focused modules?**
  _Cohesion score 0.06 - nodes in this community are weakly interconnected._
- **Should `Win32 API Primitives` be split into smaller, more focused modules?**
  _Cohesion score 0.07 - nodes in this community are weakly interconnected._