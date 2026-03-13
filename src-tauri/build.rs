fn main() {
    // On macOS, pre-compile the Swift helper so it can be bundled as a resource.
    // Build into OUT_DIR (target-specific), then copy to the source-tree staging
    // path that tauri.macos.conf.json references for bundling.
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let source = std::path::Path::new(&manifest_dir)
            .join("macos-helper/PrintQueueMacHelper.swift");

        if source.exists() {
            // Determine the swiftc -target triple from the Rust target architecture.
            // Without this, cross-compiling (e.g. x86_64 on an arm64 runner) would
            // silently produce a helper for the host arch, not the target arch.
            let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
            let swift_arch = match target_arch.as_str() {
                "x86_64" => "x86_64",
                "aarch64" => "arm64",
                _ => panic!("Unsupported macOS target architecture: {}", target_arch),
            };
            let swift_target = format!("{}-apple-macosx11.0", swift_arch);

            let build_binary = std::path::Path::new(&out_dir).join("printqueue-macos-helper");

            let output = Command::new("xcrun")
                .args([
                    "swiftc",
                    "-O",
                    "-target",
                    &swift_target,
                    "-framework",
                    "AppKit",
                    "-framework",
                    "Foundation",
                    "-framework",
                    "PDFKit",
                    &source.to_string_lossy(),
                    "-o",
                    &build_binary.to_string_lossy(),
                ])
                .output()
                .expect("Failed to run xcrun swiftc");

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("Failed to compile macOS helper: {}", stderr);
            }

            // Copy to the source-tree staging path for Tauri resource bundling.
            // This path is referenced by tauri.macos.conf.json and is gitignored.
            let staging_dir =
                std::path::Path::new(&manifest_dir).join("macos-helper");
            let staging_binary = staging_dir.join("printqueue-macos-helper");
            std::fs::copy(&build_binary, &staging_binary).unwrap_or_else(|e| {
                panic!(
                    "Failed to copy helper to staging path: {}",
                    e
                )
            });

            println!("cargo:rerun-if-changed=macos-helper/PrintQueueMacHelper.swift");
        }
    }

    tauri_build::build()
}
