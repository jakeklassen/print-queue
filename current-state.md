# PrintQueue — Current State (2026-02-23)

## Overview

All 8 phases from the implementation plan are complete. The app compiles, runs, and prints on Windows. Core functionality works end-to-end: watch folder → detect file → parse filename → match preset → submit print job. The job queue, dashboard, and system tray are all wired up.

**Next step:** Switch to macOS and continue development/testing there. CUPS is significantly more straightforward for printer control than the Windows printing stack.

---

## What Works

- **Watch folder:** File watcher via `notify` crate detects new files, debounces (1.5s), processes images and zips
- **Zip extraction:** Extracts images from .zip files, deduplicates via SHA-256 hash
- **Filename parsing:** Parses size keywords (4x6, 5x7, 8x10, 8.5x11/letter, A4-A6, aliases like 4R/KG/5R) — 6 unit tests passing
- **Preset matching:** Routes images to presets by keyword, falls back to default preset
- **Printer discovery:** Cross-platform via `printers` crate — lists name, driver, default flag
- **Printer capabilities:** Real driver options (paper sizes, media types, input slots, quality, duplex, etc.)
  - Windows: Queries Print Ticket XML via PowerShell `System.Printing`
  - macOS: Parses `lpoptions -p <printer> -l` output
- **Print submission:** Submits jobs with preset settings applied
  - Windows: PowerShell script using `System.Drawing.Printing.PrintDocument` + Print Ticket DEVMODE
  - macOS: `lp -d <printer> -o key=value` (untested, implemented)
- **Job queue:** In-memory queue with status tracking (pending/printing/complete/error/needs_attention), wired to UI
- **Dashboard:** Watch folder status, preset summary, job stats, recent activity feed
- **Queue view:** Active/history split, cancel/retry/reprint controls
- **Presets UI:** Full CRUD with printer selector, dynamic capabilities form, keyword, copies, auto-print toggle
- **Settings:** Watch folder, minimize-to-tray toggle, post-print/post-zip file handling
- **System tray:** Minimize to tray on close (configurable), tray icon with Open/Pause/Quit menu
- **UI:** React 19 + Tailwind CSS v4 + shadcn/ui (new-york style, neutral theme), dark/light mode

---

## Known Issues on Windows

### 1. Print Scaling (UNRESOLVED)
The printed output is "blown up" — image doesn't match the selected paper size (e.g., 4x6 borderless). The DEVMODE is applied to `PrintDocument` via `PrintTicketConverter` → `SetHdevmode()`, but it's unclear whether `PageBounds` in the `PrintPage` event actually reflects the custom paper size or defaults to Letter.

**Current state:** The `PrintPage` handler draws the image to fill `PageBounds` with no additional scaling. This should be correct IF the DEVMODE is properly setting the paper size. Needs further testing.

**Possible causes:**
- `PrintDocument.DefaultPageSettings.PaperSize.PaperName` still reports "Letter" after DEVMODE application (observed in testing), suggesting the .NET PaperSize object may not reflect vendor-specific sizes
- The DEVMODE may contain the correct size internally for the driver, but `PageBounds` might not update accordingly
- The `PrintTicketConverter.ConvertPrintTicketToDevMode()` might not be preserving vendor-specific (`ns0000:`) settings

**Potential fix directions:**
- Test whether `PageBounds` actually changes after DEVMODE for vendor paper sizes
- Consider setting `PrintDocument.DefaultPageSettings.PaperSize` explicitly by matching dimensions
- Consider bypassing `PrintDocument` entirely and using `PrintQueue.AddJob()` with an XPS document + PrintTicket (avoids the DEVMODE bridge entirely)
- On macOS, `lp -o PageSize=...` should just work

### 2. PowerShell Window Flashing (FIXED)
**Problem:** `powershell -WindowStyle Hidden` only minimizes the window, doesn't prevent it from appearing.
**Fix:** Use `CREATE_NO_WINDOW` (0x08000000) process creation flag via `CommandExt::creation_flags()`.

### 3. PrintTicketConverter Assembly (FIXED)
**Problem:** `System.Printing.Interop.PrintTicketConverter` was not found when loading only `System.Printing`.
**Fix:** Also load `ReachFramework` assembly — that's where the interop class actually lives.

### 4. Close-to-Tray Not Working (FIXED)
**Problem:** `api.prevent_close()` was called but the window was never hidden.
**Fix:** Clone the window handle and call `window_for_close.hide()` after preventing close.

---

## Windows Printing Architecture (for reference)

The Windows print path is the most complex part of the codebase. Here's the chain:

```
Preset settings (HashMap<String, String>)
  → keys are Print Schema feature names (e.g., "psk:PageMediaSize", "psk:PageInputBin")
  → values are Print Schema option names (e.g., "ns0000:Fullsize4x6", "ns0000:AutoSheetFeeder")

Build PowerShell script → write to temp .ps1 file
  → Load System.Printing + ReachFramework assemblies
  → Get printer's DefaultPrintTicket XML
  → Modify XML: for each setting, find/create Feature node, set Option name
  → Rebuild PrintTicket from modified XML
  → MergeAndValidatePrintTicket() against printer defaults
  → PrintTicketConverter.ConvertPrintTicketToDevMode() → byte[]
  → GlobalAlloc + Marshal.Copy → HGLOBAL
  → PrinterSettings.SetHdevmode() + PageSettings.SetHdevmode()
  → PrintDocument.Print() with PrintPage handler drawing image to PageBounds

Run via: powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File <script>
  with CREATE_NO_WINDOW flag to prevent terminal flash
```

### Crates and APIs Tried

| Approach | Status | Notes |
|----------|--------|-------|
| `printers` crate v2 (`Printer::print()`) | Works for basic print | Cannot pass custom settings (paper size, media type, etc). `PrinterJobOptions` is too limited. Also, `sha2::Digest` trait conflicts with `printers` types — use `PrinterJobOptions::none()` and `format!("{:?}", e)` for `PrintersError`. |
| `Start-Process -Verb Print` | Abandoned | Opens the default app's print dialog — not headless |
| `Start-Process -Verb PrintTo` | Not tried | Uses shell association — limited control over settings |
| PowerShell `System.Drawing.Printing.PrintDocument` | Current approach | Standard .NET print API. Works but requires DEVMODE bridging for custom settings. |
| PowerShell Print Ticket XML manipulation | Current approach | Works for modifying ticket XML. Vendor namespaces (`ns0000:`) need special handling. |
| `PrintTicketConverter` (DEVMODE bridge) | Current approach | Lives in `ReachFramework` assembly (not `System.Printing`). Converts validated PrintTicket → DEVMODE bytes. |
| P/Invoke `GlobalAlloc`/`SetHdevmode` | Current approach | Bridges DEVMODE bytes into `PrinterSettings`/`PageSettings`. The DEVMODE is applied but .NET `PaperSize.PaperName` may not reflect vendor sizes. |
| `PrintQueue.AddJob()` with XPS | Not tried | Would bypass PrintDocument/DEVMODE entirely. More complex (need to create XPS doc with WPF) but might handle settings more reliably. |
| `winprint` crate | Not tried | Mentioned in PRD as option but not used. |

### Epson ET-8500 Specifics (Windows)

Discovered via `System.Printing.PrintCapabilities`:
- **4x6 paper:** `ns0000:Fullsize4x6` (standard), probably a `Fullbleed` variant for borderless
- **Rear feed (with backing support):** `ns0000:AutoSheetFeeder`
- **Rear single-sheet slot:** `ns0000:RearManualFeeder`
- Print Schema namespace: `psk:` for standard, `ns0000:` for Epson vendor options

---

## macOS Printing (Ready to Implement)

The macOS path (`submit_print_unix`) is already implemented and much simpler:

```rust
let mut cmd = Command::new("lp");
cmd.arg("-d").arg(printer_id);
cmd.arg("-n").arg(preset.copies.to_string());
for (key, value) in &preset.settings {
    cmd.arg("-o").arg(format!("{}={}", key, value));
}
cmd.arg(file_path);
```

Capabilities are queried via `lpoptions -p <printer> -l` which returns all options with their choices and defaults. Settings map directly to `-o key=value` on the `lp` command. No DEVMODE, no Print Ticket XML, no P/Invoke.

**Expected macOS issues:** None architectural — CUPS is well-documented and the `lp` command handles settings natively. May need to test:
- Whether the `printers` crate works on macOS for discovery (it should)
- Whether `lpoptions` output parsing handles all printer types
- Orientation handling (already added as a synthetic option if not in `lpoptions` output)

---

## Project Structure

```
src/                              # React frontend
  App.tsx                         # Root: ThemeProvider, sidebar nav, 4 views
  App.css                         # Tailwind imports
  components/
    app-sidebar.tsx               # Icon sidebar with theme toggle
    capabilities-form.tsx         # Dynamic printer options from capabilities
    preset-form.tsx               # Create/edit preset dialog
    printer-selector.tsx          # Printer dropdown with refresh
    theme-provider.tsx            # Dark/light/system theme
    ui/                           # shadcn components (button, card, select, badge, etc.)
  lib/
    api.ts                        # All Tauri IPC wrappers
    types.ts                      # TypeScript types mirroring Rust models
    utils.ts                      # cn() utility for shadcn
  pages/
    dashboard.tsx                 # Watch status, presets, job stats, recent activity
    presets.tsx                   # CRUD preset management
    queue.tsx                     # Job list with status, cancel/retry/reprint
    settings.tsx                  # Watch folder, tray, file handling options

src-tauri/                        # Rust backend
  src/
    lib.rs                        # App setup: plugins, state, tray, close-to-tray, auto-start
    main.rs                       # Binary entry point
    commands.rs                   # All #[tauri::command] handlers
    models.rs                     # AppConfig, Preset, PrintSettings (HashMap), PostFileAction
    storage.rs                    # JSON file persistence to app data dir
    printing.rs                   # Printer discovery, capabilities (PowerShell/lpoptions)
    watcher.rs                    # File watcher, zip extraction, preset routing, print submission
    parser.rs                     # Filename keyword parser with tests
    jobs.rs                       # In-memory job queue (PrintJob, JobQueueState)
    tray.rs                       # System tray setup
  Cargo.toml                      # Rust dependencies
  tauri.conf.json                 # Window: 900x650, min 680x480
  capabilities/default.json       # Tauri v2 permissions

CLAUDE.md                         # Coding instructions and project overview
PRD.md                            # Full product requirements
```

## Key Dependencies

### Rust (Cargo.toml)
- `tauri` 2 (with `tray-icon`, `image-png` features)
- `tauri-plugin-dialog` 2, `tauri-plugin-notification` 2, `tauri-plugin-opener` 2
- `printers` 2 — cross-platform printer enumeration
- `notify` 8 — filesystem watching
- `zip` 2 — zip extraction
- `image` 0.25 — format validation (jpeg, png, tiff)
- `sha2` 0.10 + `hex` 0.4 — file hashing for dedup
- `serde` 1 + `serde_json` 1 — serialization
- `uuid` 1 + `chrono` 0.4 — IDs and timestamps

### Frontend (package.json)
- React 19, TypeScript, Vite 7
- Tailwind CSS v4 + `@tailwindcss/vite`
- shadcn/ui components (new-york style, neutral base color)
- `@tauri-apps/api`, `@tauri-apps/plugin-dialog`
- `lucide-react` for icons

## Data Storage

JSON files in Tauri app data directory:
- `config.json` — `AppConfig` (watch folder, tray behavior, default preset, post-file actions)
- `presets.json` — `Vec<Preset>` (name, printer, keyword, settings HashMap, copies, auto-print)

Settings use `HashMap<String, String>` where keys are Print Schema feature names and values are option names. This is platform-agnostic — on macOS, the same keys map to CUPS option names from `lpoptions`.
