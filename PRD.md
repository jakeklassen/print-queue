# PrintQueue — Product Requirements Document

## Overview

PrintQueue is a cross-platform desktop application built with Tauri that automates photo printing workflows for makers and small business owners. Users configure a watch folder, select their printer and settings, and the app automatically detects new zip files, extracts images, and sends them to the printer with the correct preset configuration — eliminating the manual process of unzipping, opening print software, configuring settings, and drag-and-dropping files for every batch.

## Problem Statement

Makers who print photos, magnets, stickers, and other physical products currently rely on manual workflows involving tools like Epson Photo+ where they must unzip downloaded files, open a print application, configure print settings, and drag-and-drop images into a print zone for every batch. This is time-consuming, error-prone (wrong settings get applied), and creates friction in an otherwise streamlined production pipeline.

## Target Users

- Small business makers (photo magnets, stickers, prints, etc.)
- Home photo printing enthusiasts
- Small print shops with repetitive print jobs
- Primarily Windows users, with macOS support required

## Platform Support

- **Windows** (primary — majority of maker community)
- **macOS** (secondary — required for MVP)

---

## Core Architecture

### Technology Stack

- **Framework:** Tauri v2
- **Frontend:** React with TypeScript (TanStack Router if multi-view)
- **Backend:** Rust
- **Styling:** Tailwind CSS + shadcn/ui

### Rust Crates (Key Dependencies)

| Crate                  | Purpose                                                                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------------------- |
| `notify`               | Cross-platform filesystem watching                                                                       |
| `zip`                  | Zip file extraction                                                                                      |
| `printers`             | Cross-platform printer discovery and job submission (CUPS on macOS, winspool on Windows)                 |
| `winprint`             | Windows-specific printer capability querying and Print Ticket support (Windows only, compile-time gated) |
| `cups_rs`              | macOS/Linux-specific printer capability querying (macOS only, compile-time gated)                        |
| `serde` / `serde_json` | Preset and configuration serialization                                                                   |
| `image`                | Image format detection and validation                                                                    |

### Platform Abstraction

A unified Rust trait should abstract over platform-specific printing implementations:

```
trait PrintService {
    fn discover_printers() -> Vec<PrinterInfo>;
    fn get_printer_capabilities(printer_id: &str) -> PrinterCapabilities;
    fn print_file(printer_id: &str, file_path: &Path, options: &PrintOptions) -> Result<JobId>;
    fn get_job_status(job_id: JobId) -> JobStatus;
}
```

- **macOS implementation:** Uses `cups_rs` for capability enumeration and `lp` / CUPS API for job submission
- **Windows implementation:** Uses `winprint` for capability querying and Print Ticket-based job submission, falling back to `printers` crate (winspool/PowerShell) where needed

Conditional compilation via `#[cfg(target_os = "macos")]` and `#[cfg(target_os = "windows")]` keeps platform code isolated.

---

## Features

### 1. Watch Folder & Two-Stage Pipeline

**Description:** The app monitors a user-selected folder using a two-stage pipeline. The image watcher is the single entry point for all printing — zip extraction is just a convenience layer that feeds into it.

**Pipeline:**

```
Watch Folder
    ├── .zip detected  →  Stage 1: Extract images into watch folder  →  done
    └── .jpg/.png/.tiff detected  →  Stage 2: Parse filename → route to preset → print
```

**Why two stages:** Not all print jobs arrive as zips. Single image downloads, manual file drops, or images written by other tools should all trigger printing. By making the image file itself the print trigger (not the zip), the system handles every input method through a single code path.

**Requirements:**

#### Folder Configuration

- Native folder picker dialog via Tauri's dialog API
- Persist selected folder path across app restarts (stored in Tauri app data)
- Display the currently watched folder in the UI
- Ability to change the watch folder at any time
- Watcher should start automatically on app launch if a folder is configured
- Handle edge cases: folder deleted, permissions changed, drive unmounted

#### Stage 1: Zip Extraction

- On detecting a new `.zip` file, wait briefly (1-2 seconds) to ensure the file has finished downloading/copying
- Extract all supported image files (JPEG, PNG, TIFF) into the watch folder (or a configurable subfolder)
- Non-image files within the zip are ignored
- Optionally delete or move the `.zip` file after successful extraction
- Track processed zip files (by filename + hash) to avoid re-extracting on app restart

#### Stage 2: Image Detection & Printing

- On detecting a new image file (`.jpg`, `.jpeg`, `.png`, `.tiff`), wait briefly to ensure the file has finished writing
- Parse the filename for size keywords (see File Naming Convention System)
- Route to the matching preset
- Submit the print job
- After successful print, move the image to a "printed" subfolder or delete (configurable). The "printed" subfolder doubles as a reprint archive.
- Tracking only prevents double-processing of files currently in the queue or actively printing. Once a job completes, the filename is eligible for reprinting — re-downloading, duplicates, or moving the file back from the "printed" subfolder all trigger new jobs naturally.
- This stage triggers identically whether the image came from a zip extraction or was placed in the folder by any other means

### 2. Printer Discovery & Selection

**Description:** The app enumerates all available printers on the system and presents them in the UI for selection.

**Requirements:**

- List all system printers with display name and driver info
- Indicate the system default printer
- Show printer status (online/offline/error) where available
- Refresh button to re-enumerate printers
- Selected printer is saved to the active preset

### 3. Printer Capability Enumeration

**Description:** Once a printer is selected, query its available settings and present them as configurable options in the UI.

**Requirements:**

- Dynamically enumerate capabilities from the selected printer — do not hardcode options
- Display available options for at minimum:
  - **Paper/document size** (e.g., 4x6, 5x7, 8x10, 8.5x11, and borderless/fullbleed variants)
  - **Paper source / input tray** (e.g., rear paper feeder, cassette)
  - **Paper/media type** (e.g., photo paper glossy, plain paper)
  - **Print quality** (e.g., draft, standard, high, best photo)
  - **Color mode** (e.g., color, black/grayscale)
  - **Orientation** (portrait, landscape, auto-detect based on image dimensions)
- Each option renders as a dropdown populated with values reported by the printer driver
- If a capability is not available for a given printer, hide that option in the UI

### 4. Print Presets

**Description:** Users can create named presets that bundle a printer, its settings, and filename pattern matching rules. This is the core of the automation — different files route to different presets.

**Requirements:**

- A preset consists of:
  - **Name** (user-defined, e.g., "4x6 Photo Magnets", "8.5x11 Sticker Sheets")
  - **Printer** (selected from discovered printers)
  - **Paper size keyword** (which filename keyword routes to this preset, e.g., `4x6`)
  - **Print settings** (all options from capability enumeration)
  - **Copies** (default number of copies per image)
  - **Auto-print** (boolean — print immediately or require confirmation)
  - **Scale compensation factor** (percentage, default 100% — see Calibration)
- CRUD operations: create, edit, duplicate, delete presets
- Presets stored as JSON in Tauri app data directory
- One preset can be marked as the "default" — used when no filename size keyword matches
- Preset routing is driven by the File Naming Convention System (see Feature 5)

### 5. File Naming Convention System

**Description:** The file naming convention is a core mechanism of the app. Image filenames encode print instructions — most importantly the paper size. The app parses filenames to automatically determine how each image should be printed, routing it to the correct preset.

**Requirements:**

#### Built-in Size Keywords

The app ships with a set of recognized paper size keywords that can appear anywhere in the filename:

| Keyword(s)         | Dimensions                     | Notes                  |
| ------------------ | ------------------------------ | ---------------------- |
| `4x6`, `4R`, `KG`  | 4 × 6 in (101.6 × 152.4 mm)    | Most common photo size |
| `5x7`, `5R`        | 5 × 7 in (127 × 177.8 mm)      |                        |
| `8x10`             | 8 × 10 in (203.2 × 254 mm)     |                        |
| `8.5x11`, `letter` | 8.5 × 11 in (215.9 × 279.4 mm) | US Letter              |
| `A4`               | 210 × 297 mm                   | ISO standard           |
| `A5`               | 148 × 210 mm                   | ISO standard           |
| `A6`               | 105 × 148 mm                   | ISO standard           |

- Keywords are case-insensitive (`4X6`, `4x6`, `a4`, `A4` all match)
- Keywords can be delimited by spaces, underscores, hyphens, or dots (e.g., `order123_4x6.png`, `my-photo 4x6.jpg`, `batch.4x6.001.png`)

#### Custom Keywords

- Users can create custom size keywords that map to specific paper dimensions
- Example: a user might define `magnet` → 4 × 6 in, or `jumbo` → 8 × 10 in
- Custom keywords are matched with the same rules as built-in keywords
- Custom keywords take priority over built-in keywords if there's a conflict

#### Filename Parsing Rules

- The parser scans the filename (excluding extension) for recognized keywords
- If a size keyword is found, the image is routed to the preset matching that paper size
- If multiple size keywords are found in a filename, the first match wins
- If no size keyword is found, the image is routed to the default preset
- If no default preset exists and no keyword matches, the file is queued with a "needs attention" status and the user is notified

#### Preset Matching Flow

```
filename: "order_839_magnet_4x6_002.png"
                            ^^^
                        keyword found: 4x6
                            ↓
            find preset configured for 4x6 paper size
                            ↓
                print with that preset's settings
```

#### UI for Naming Conventions

- A dedicated section in Settings showing all recognized keywords (built-in + custom)
- Ability to add/edit/delete custom keywords
- Each keyword shows its mapped paper size dimensions
- A "test filename" input where users can type a filename and see which preset it would route to
- Tooltip/help text explaining the naming convention for users to share with their upstream tools or order systems

### 6. Print Calibration

**Description:** A calibration workflow to determine the scale compensation factor for a given printer, accounting for driver-level scaling inconsistencies inherent in consumer printers.

**Requirements:**

- Guided calibration flow:
  1. App prints a test image with known reference dimensions (e.g., a rectangle of known mm size with measurement markers)
  2. User measures the printed output with a ruler
  3. User enters the measured dimensions into the app
  4. App calculates the compensation factor (expected / measured) and saves it to the preset
- Compensation factor is applied when composing the print job (scale the image by the factor before sending)
- Per-preset calibration (different printers/paper sizes may have different factors)
- Ability to re-calibrate at any time
- Display current calibration factor in preset settings

### 7. Job Queue & Status

**Description:** A visible queue showing pending, active, and completed print jobs with image previews and reprint capability.

**Requirements:**

#### Queue Display

- Each job shows: image thumbnail preview, filename, preset used, status (pending / printing / complete / error), timestamp
- Clicking the thumbnail opens a larger preview within the app for quick visual verification
- Double-click or "Open file" button opens the image in the system's default image viewer for full-resolution inspection
- Real-time status updates as jobs process

#### Reprint Workflow

- **From the UI:** A reprint button on any completed or failed job. Resubmits the same file with the same preset — no file manipulation needed. This is the fastest path when standing at the printer and spotting a defect.
- **From the folder:** The watcher does not permanently track processed files. Tracking only prevents double-processing of files currently in the queue or actively printing. Once a job completes and the image moves to the "printed" subfolder, that filename is eligible again. Re-downloading the file, dragging it back from the "printed" subfolder, or a duplicate landing (e.g., `photo_4x6 (1).png`) all trigger a new print job naturally.
- The "printed" subfolder serves double duty: cleanup and reprint archive.

#### Job Controls

- Cancel pending jobs
- Retry failed jobs
- Reprint completed jobs
- Toast/system notification on job completion or error
- Basic job history (persisted, clearable)

### 8. System Tray Integration

**Description:** The app should be able to minimize to the system tray and continue watching/printing in the background.

**Requirements:**

- Minimize to tray on window close (configurable — can also fully quit)
- Tray icon with status indicator (idle, processing, error)
- Tray context menu: open window, pause/resume watching, quit
- Desktop notification on print completion or errors when minimized

---

## UI Structure

### Views

1. **Dashboard / Home**
   - Current watch folder (with change button)
   - Watcher status (active/paused)
   - Active printer and preset summary
   - Recent job activity feed with image thumbnails
   - Click thumbnail for in-app preview, double-click to open in system viewer

2. **Presets Management**
   - List of all presets with quick summary (printer, paper size, pattern)
   - Create / edit / duplicate / delete
   - Drag to reorder (affects pattern matching priority)
   - Calibration trigger per preset

3. **Print Queue**
   - Active and pending jobs
   - Job history
   - Cancel / retry controls

4. **Settings**
   - Watch folder configuration
   - System tray behavior
   - Startup on login toggle
   - Notification preferences

### Design Principles

- Clean, minimal UI — this is a utility app, not a creative tool
- Status-forward: always clear what the app is doing (watching, printing, idle, error)
- Progressive disclosure: simple view by default, advanced options available but not overwhelming
- Dark and light mode support

---

## Data Model

### AppConfig

```json
{
  "watch_folder": "/path/to/folder",
  "minimize_to_tray": true,
  "start_on_login": false,
  "default_preset_id": "uuid"
}
```

### Preset

```json
{
  "id": "uuid",
  "name": "4x6 Photo Magnets",
  "printer_id": "ET-8500 Series(Network)",
  "paper_size_keyword": "4x6",
  "settings": {
    "paper_size": "101.6x180.6mm.Fullbleed",
    "paper_source": "RearPaperFeeder",
    "paper_type": "PhotoPaperGlossy",
    "quality": "Best",
    "color_mode": "Color",
    "orientation": "auto",
    "borderless": true
  },
  "copies": 1,
  "auto_print": true,
  "scale_compensation": 1.015,
  "created_at": "ISO8601",
  "updated_at": "ISO8601"
}
```

### CustomKeyword

```json
{
  "keyword": "magnet",
  "paper_width_mm": 101.6,
  "paper_height_mm": 152.4,
  "maps_to_preset_id": "uuid"
}
```

### ProcessedFile (tracking to avoid reprints)

```json
{
  "filename": "batch_001.zip",
  "hash": "sha256",
  "processed_at": "ISO8601",
  "job_count": 12,
  "preset_id": "uuid"
}
```

---

## Non-Functional Requirements

- **Performance:** Zip extraction and print job queuing should feel instant for typical batch sizes (1-50 images). File watching should use OS-native events (inotify/FSEvents/ReadDirectoryChanges), not polling.
- **Reliability:** Graceful handling of printer going offline mid-batch, paper jams, zip files containing non-image files, corrupted images. Failed jobs should be retryable.
- **Storage:** All config and presets stored in the standard Tauri app data directory. No external database.
- **Packaging:** Distribute as `.dmg` for macOS and `.msi` installer for Windows via Tauri's built-in bundler.
- **Auto-update:** Tauri's built-in updater for future releases.

---

## MVP Scope

The MVP includes:

- [x] Watch folder selection and monitoring
- [x] Two-stage pipeline: zip extraction feeds image watcher, images are the single print trigger
- [x] Zip extraction and image detection
- [x] Printer discovery and selection (Windows + macOS)
- [x] Dynamic printer capability enumeration
- [x] Preset creation with full print settings
- [x] File naming convention parsing with built-in size keywords
- [x] Filename-to-preset routing based on size keywords
- [x] Configurable auto-print or confirm-before-printing per preset
- [x] Automatic print job submission
- [x] Basic job queue with status, image previews, and reprint button
- [x] System tray with background operation

### Deferred to V2

- [ ] Custom user-defined keywords
- [ ] Print calibration workflow with guided test print
- [ ] "Test filename" tool in settings for verifying routing
- [ ] Dry-run / preview mode before printing a batch
- [ ] Multi-printer routing (same file to multiple printers)
- [ ] Start on login
- [ ] Auto-update
- [ ] Job history persistence

---

## Decisions

1. **Image preparation:** Images are assumed print-ready. No resizing, cropping, or bleed processing. Users are responsible for preparing images at the correct dimensions before dropping them into the watch folder.
2. **PDF intermediary:** No. Images are sent directly to the printer.
3. **Multiple printers per preset:** Not in MVP. Future consideration — file naming conventions will be important for routing to different printers.
4. **Batch acknowledgment:** Configurable per preset. Options: auto-print immediately, or require confirmation before printing a batch.
