fn main() {
    // On macOS, pre-compile the Swift helper so it can be bundled as a resource
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let source = std::path::Path::new(&manifest_dir).join("macos-helper/PrintQueueMacHelper.swift");
        let out_dir = std::path::Path::new(&manifest_dir).join("macos-helper");
        let binary = out_dir.join("printqueue-macos-helper");

        if source.exists() {
            let output = Command::new("xcrun")
                .args([
                    "swiftc",
                    "-O",
                    "-framework",
                    "AppKit",
                    "-framework",
                    "Foundation",
                    "-framework",
                    "PDFKit",
                    source.to_string_lossy().as_ref(),
                    "-o",
                    binary.to_string_lossy().as_ref(),
                ])
                .output()
                .expect("Failed to run xcrun swiftc");

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("Failed to compile macOS helper: {}", stderr);
            }

            println!("cargo:rerun-if-changed=macos-helper/PrintQueueMacHelper.swift");
        }
    }

    tauri_build::build()
}
