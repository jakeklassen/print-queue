# PrintQueue

A cross-platform desktop application built with Tauri that automates photo printing workflows for makers and small business owners. Users configure a watch folder, select their printer and settings, and the app automatically detects new zip files, extracts images, and sends them to the printer with the correct preset configuration.

## Tech Stack

- **Framework:** Tauri v2
- **Frontend:** React 19 + TypeScript, Vite 7
- **Backend:** Rust
- **Styling:** Tailwind CSS + shadcn/ui
- **Platforms:** Windows (primary), macOS (secondary)

## Prerequisites

Install the required toolchain via [mise](https://mise.jdx.dev/) (`mise install`) or manually:

- Node.js 24.13.1
- Rust 1.93.1
- pnpm 10.30.1

## Getting Started

```bash
# Install dependencies
pnpm install

# Run the full Tauri app in dev mode
pnpm tauri dev

# Build distributable app (.msi / .dmg)
pnpm tauri build
```

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
