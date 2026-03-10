#[cfg(target_os = "macos")]
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
use crate::models::Preset;
#[cfg(target_os = "macos")]
use crate::printing::PrinterInfo;
#[cfg(target_os = "macos")]
use lopdf::dictionary;

#[cfg(target_os = "macos")]
const HELPER_SOURCE: &str = "macos-helper/PrintQueueMacHelper.swift";

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacOSPrintConfiguration {
    pub printer_name: String,
    pub print_info_base64: String,
    pub page_format_base64: String,
    pub print_settings_base64: String,
}

#[cfg(target_os = "macos")]
struct MacOSSampleContext {
    sample_file: Option<PathBuf>,
    paper_size_points: Option<(f64, f64)>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Deserialize)]
struct HelperPrinterInfo {
    id: String,
    name: String,
    is_default: bool,
}

#[cfg(target_os = "macos")]
fn helper_binary_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))?;
    let helper_dir = data_dir.join("macos-helper");
    fs::create_dir_all(&helper_dir).map_err(|e| format!("Failed to create helper dir: {}", e))?;
    Ok(helper_dir.join("printqueue-macos-helper"))
}

#[cfg(target_os = "macos")]
fn helper_source_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(HELPER_SOURCE)
}

#[cfg(target_os = "macos")]
fn ensure_helper_built(app: &AppHandle) -> Result<PathBuf, String> {
    let source = helper_source_path();
    let binary = helper_binary_path(app)?;

    let should_build = match (fs::metadata(&source), fs::metadata(&binary)) {
        (Ok(source_meta), Ok(binary_meta)) => {
            source_meta.modified().ok() > binary_meta.modified().ok()
        }
        (Ok(_), Err(_)) => true,
        (Err(e), _) => return Err(format!("Missing macOS helper source: {}", e)),
    };

    if should_build {
        let swift_module_cache = std::env::temp_dir().join("printqueue-swift-module-cache");
        let clang_module_cache = std::env::temp_dir().join("printqueue-clang-module-cache");
        fs::create_dir_all(&swift_module_cache)
            .map_err(|e| format!("Failed to create Swift module cache: {}", e))?;
        fs::create_dir_all(&clang_module_cache)
            .map_err(|e| format!("Failed to create Clang module cache: {}", e))?;

        let output = Command::new("xcrun")
            .env("SWIFT_MODULECACHE_PATH", &swift_module_cache)
            .env("CLANG_MODULE_CACHE_PATH", &clang_module_cache)
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
            .map_err(|e| format!("Failed to compile macOS helper: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("macOS helper compile failed: {}", stderr.trim()));
        }
    }

    Ok(binary)
}

#[cfg(target_os = "macos")]
fn run_helper(app: &AppHandle, args: &[String]) -> Result<String, String> {
    let helper = ensure_helper_built(app)?;
    let output = Command::new(helper)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to launch macOS helper: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.trim().is_empty() {
        eprintln!("[PrintQueue][macOS] helper stdout: {}", stdout.trim());
    }
    if !stderr.trim().is_empty() {
        eprintln!("[PrintQueue][macOS] helper stderr: {}", stderr.trim());
    }

    if !output.status.success() {
        let stderr = stderr.trim();
        let concise = stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or(stderr);
        return Err(if concise.is_empty() {
            format!("macOS helper exited with code {:?}", output.status.code())
        } else {
            concise.to_string()
        });
    }

    String::from_utf8(output.stdout).map_err(|e| format!("Invalid macOS helper output: {}", e))
}

#[cfg(target_os = "macos")]
pub fn list_printers(app: &AppHandle) -> Result<Vec<PrinterInfo>, String> {
    let stdout = run_helper(app, &[String::from("list-printers")])?;
    let printers: Vec<HelperPrinterInfo> = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("Invalid printer list JSON: {}", e))?;

    Ok(printers
        .into_iter()
        .map(|printer| PrinterInfo {
            id: printer.id.clone(),
            name: printer.name,
            driver: String::new(),
            is_default: printer.is_default,
            is_online: true,
        })
        .collect())
}

#[cfg(target_os = "macos")]
pub fn configure_printer(
    app: &AppHandle,
    printer_hint: Option<&str>,
    paper_size_keyword: Option<&str>,
) -> Result<MacOSPrintConfiguration, String> {
    let sample = sample_context_for_keyword(paper_size_keyword);
    let mut args = vec![String::from("configure")];
    if let Some(printer_hint) = printer_hint.filter(|value| !value.trim().is_empty()) {
        args.push(String::from("--printer"));
        args.push(printer_hint.to_string());
    }
    if let Some(sample_file) = sample.sample_file {
        args.push(String::from("--sample-file"));
        args.push(sample_file.display().to_string());
    }
    if let Some((width, height)) = sample.paper_size_points {
        args.push(String::from("--paper-width"));
        args.push(width.to_string());
        args.push(String::from("--paper-height"));
        args.push(height.to_string());
    }

    let stdout = run_helper(app, &args)?;
    serde_json::from_str(stdout.trim()).map_err(|e| format!("Invalid configure JSON: {}", e))
}

#[cfg(target_os = "macos")]
pub fn print_file(app: &AppHandle, file_path: &Path, preset: &Preset) -> Result<(), String> {
    let print_info_base64 = preset
        .macos_print_info_base64
        .as_ref()
        .ok_or_else(|| "Preset is missing macOS native printer settings".to_string())?;
    let page_format_base64 = preset
        .macos_page_format_base64
        .as_ref()
        .ok_or_else(|| "Preset is missing macOS page format settings".to_string())?;
    let print_settings_base64 = preset
        .macos_print_settings_base64
        .as_ref()
        .ok_or_else(|| "Preset is missing macOS print settings".to_string())?;

    let pdf_path = wrap_image_in_pdf(file_path, preset)?;

    let mut args = vec![
        String::from("print"),
        String::from("--file"),
        pdf_path.display().to_string(),
        String::from("--copies"),
        preset.copies.to_string(),
        String::from("--print-info-b64"),
        print_info_base64.clone(),
        String::from("--page-format-b64"),
        page_format_base64.clone(),
        String::from("--print-settings-b64"),
        print_settings_base64.clone(),
    ];

    if let Some(printer_name) = preset
        .macos_printer_name
        .as_ref()
        .or(preset.printer_id.as_ref())
        .filter(|value| !value.trim().is_empty())
    {
        args.push(String::from("--printer"));
        args.push(printer_name.to_string());
    }

    let result = run_helper(app, &args);
    fs::remove_file(&pdf_path).ok();
    result?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn sample_context_for_keyword(keyword: Option<&str>) -> MacOSSampleContext {
    let normalized = keyword.unwrap_or("").trim().to_lowercase();
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let sample_file = match normalized.as_str() {
        "4x6" | "kg" | "4r" => Some(repo_root.join("public/4x6.jpg")),
        _ => None,
    }
    .filter(|path| path.exists());

    let paper_size_points = match normalized.as_str() {
        "4x6" | "kg" | "4r" => Some((288.0, 432.0)),
        "5x7" | "5r" | "2l" => Some((360.0, 504.0)),
        "8x10" => Some((576.0, 720.0)),
        "3.5x5" | "l" => Some((252.0, 360.0)),
        "letter" | "8.5x11" => Some((612.0, 792.0)),
        "legal" => Some((612.0, 1008.0)),
        "a4" => Some((595.28, 841.89)),
        "a6" => Some((297.64, 419.53)),
        _ => None,
    };

    MacOSSampleContext {
        sample_file,
        paper_size_points,
    }
}

#[cfg(target_os = "macos")]
fn paper_size_points(keyword_or_page_size: &str) -> Option<(f32, f32)> {
    let base = keyword_or_page_size
        .split('.')
        .next()
        .unwrap_or(keyword_or_page_size);
    match base.to_lowercase().as_str() {
        "epkg" | "4x6" | "kg" | "4r" => Some((288.0, 432.0)),
        "epphotopaper2l" | "5x7" | "5r" | "2l" => Some((360.0, 504.0)),
        "ep8x10in" | "8x10" => Some((576.0, 720.0)),
        "epphotopaperlroll" | "3.5x5" | "l" => Some((252.0, 360.0)),
        "ephivision102x180" => Some((289.1, 510.2)),
        "letter" | "8.5x11" => Some((612.0, 792.0)),
        "legal" => Some((612.0, 1008.0)),
        "a4" => Some((595.28, 841.89)),
        "a6" => Some((297.64, 419.53)),
        "ephalfletter" | "statement" => Some((396.0, 612.0)),
        "env10" => Some((297.0, 684.0)),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn wrap_image_in_pdf(image_path: &Path, preset: &Preset) -> Result<PathBuf, String> {
    use lopdf::{Document, Object, Stream};

    let page_size_key = preset
        .settings
        .get("PageSize")
        .map(|s| s.as_str())
        .unwrap_or_else(|| preset.paper_size_keyword.as_str());
    let (width_pt, height_pt) = paper_size_points(page_size_key).unwrap_or((288.0, 432.0));

    let orientation = preset
        .settings
        .get("orientation-requested")
        .map(|s| s.as_str());
    let (width_pt, height_pt) = match orientation {
        Some("4") | Some("5") => (height_pt, width_pt),
        _ => (width_pt, height_pt),
    };

    let image_scale = preset.scale_compensation.max(0.01) as f32;
    let img_width = width_pt * image_scale;
    let img_height = height_pt * image_scale;
    let offset_x = (width_pt - img_width) / 2.0;
    let offset_y = (height_pt - img_height) / 2.0;

    let image_bytes = fs::read(image_path).map_err(|e| format!("Failed to read image: {}", e))?;
    let img = ::image::load_from_memory(&image_bytes)
        .map_err(|e| format!("Failed to decode image: {}", e))?;
    let (img_w, img_h) = ::image::GenericImageView::dimensions(&img);

    let ext = image_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let (stream_bytes, filter) = if ext == "jpg" || ext == "jpeg" {
        (image_bytes, "DCTDecode")
    } else {
        let rgb = img.to_rgb8();
        let raw_pixels = rgb.into_raw();
        let compressed = {
            use flate2::write::ZlibEncoder;
            use flate2::Compression;
            use std::io::Write;
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
            enc.write_all(&raw_pixels)
                .map_err(|e| format!("Compression error: {}", e))?;
            enc.finish()
                .map_err(|e| format!("Compression error: {}", e))?
        };
        (compressed, "FlateDecode")
    };

    let mut doc = Document::with_version("1.4");

    let image_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => img_w as i64,
            "Height" => img_h as i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8_i64,
            "Filter" => filter,
        },
        stream_bytes,
    );
    let image_id = doc.add_object(image_stream);

    let content = format!(
        "q\n{:.4} 0 0 {:.4} {:.4} {:.4} cm\n/Img1 Do\nQ\n",
        img_width, img_height, offset_x, offset_y,
    );
    let content_stream = Stream::new(
        dictionary! { "Length" => content.len() as i64 },
        content.into_bytes(),
    );
    let content_id = doc.add_object(content_stream);

    let resources = dictionary! {
        "XObject" => dictionary! { "Img1" => image_id },
    };

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![0.into(), 0.into(), Object::Real(width_pt), Object::Real(height_pt)],
        "Contents" => content_id,
        "Resources" => resources,
    });

    let pages_id = doc.add_object(dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    });

    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Object::Dictionary(ref mut dict) = page_obj {
            dict.set("Parent", pages_id);
        }
    }

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let pdf_path = std::env::temp_dir().join(format!(
        "printqueue-native-{}.pdf",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));

    doc.save(&pdf_path)
        .map_err(|e| format!("Failed to save native PDF: {}", e))?;
    Ok(pdf_path)
}
