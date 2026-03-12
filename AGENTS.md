# Repository Guidelines

## Project Structure & Module Organization

- `src/`: React + TypeScript frontend. Entry at `src/main.tsx`, root UI in `src/App.tsx`.
- `src-tauri/`: Rust backend and Tauri configuration (`src-tauri/src/lib.rs`, `src-tauri/src/main.rs`, `src-tauri/tauri.conf.json`).
- `public/`: Static assets served by Vite.
- `dist/`: Frontend build output (generated).
- `PRD.md`: Product requirements and workflow details.

## Build, Test, and Development Commands

- `pnpm install`: Install JS dependencies.
- `pnpm dev`: Run the Vite dev server (frontend only).
- `pnpm tauri dev`: Run the full Tauri app (frontend + Rust backend).
- `pnpm build`: Type-check (`tsc`) and bundle the frontend.
- `pnpm tauri build`: Build distributable desktop apps.

## Coding Style & Naming Conventions

- TypeScript is in strict mode; fix type errors rather than silencing them.
- Indentation is 2 spaces in TS/TSX and CSS. Use double quotes in TS/TSX imports and strings as seen in the codebase.
- Rust should follow `rustfmt` defaults.
- Naming: React components in `PascalCase`, hooks in `useX`, files in `kebab-case` or `lowercase` per existing folders.
- No linter/formatter is configured; keep changes consistent with nearby code.

## Testing Guidelines

- No test framework is configured yet. If you add one, update this file with the framework, test locations, and run commands.

## Commit & Pull Request Guidelines

- Commit history uses short, sentence-case or lowercase summaries without a strict convention. Keep messages concise and descriptive (e.g., "add queue filter" or "fix preset save").
- PRs should include:
  - A clear summary of what changed and why.
  - Testing notes (commands run or "not run" with reason).
  - UI changes should include before/after screenshots or a short screen capture.

## Configuration & Security Notes

- Toolchain versions are tracked in `mise.toml`. Align local versions to avoid build mismatches.
- Tauri permissions are defined in `src-tauri/capabilities/`; review before adding new APIs.
