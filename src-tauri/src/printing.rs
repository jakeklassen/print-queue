use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub is_default: bool,
    pub is_online: bool,
}

/// A single printer option (e.g. "PageSize", "MediaType", "InputSlot").
/// Each has a list of choices and an optional default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterOption {
    /// Internal key (e.g. "PageSize", "MediaType", "InputSlot")
    pub key: String,
    /// Human-readable label (e.g. "Media Size", "Media Type", "Media Source")
    pub label: String,
    /// Available choices, in order
    pub choices: Vec<PrinterOptionChoice>,
    /// The key of the default choice, if any
    pub default_choice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterOptionChoice {
    /// Internal value (e.g. "4x6.Fullbleed", "photographic-glossy")
    pub value: String,
    /// Display label — same as value unless we can derive a better one
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterCapabilities {
    /// All options reported by the driver, keyed by option key
    pub options: Vec<PrinterOption>,
}

pub fn discover_printers() -> Vec<PrinterInfo> {
    // On macOS/Linux, use lpstat to get the real CUPS queue names.
    // The `printers` crate returns display names (with spaces/hyphens) that
    // don't work with lpoptions/lp commands which need the queue name
    // (underscored form).
    #[cfg(not(target_os = "windows"))]
    {
        discover_printers_cups()
    }
    #[cfg(target_os = "windows")]
    {
        discover_printers_windows()
    }
}

#[cfg(target_os = "windows")]
fn discover_printers_windows() -> Vec<PrinterInfo> {
    let system_printers = printers::get_printers();
    let default_printer = printers::get_default_printer();
    let default_name = default_printer.as_ref().map(|p| p.name.clone());

    system_printers
        .into_iter()
        .map(|p| {
            let is_default = default_name.as_deref() == Some(&p.name);
            PrinterInfo {
                id: p.name.clone(),
                name: p.name.clone(),
                driver: p.driver_name,
                is_default,
                is_online: true,
            }
        })
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn discover_printers_cups() -> Vec<PrinterInfo> {
    use std::process::Command;

    let mut printers_list = Vec::new();

    // Get default printer
    let default_name = Command::new("lpstat")
        .args(["-d"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                // Output: "system default destination: PRINTER_NAME"
                text.split(':').nth(1).map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    // Get all printers via lpstat -p
    let output = Command::new("lpstat")
        .args(["-p"])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                // Lines look like: "printer QUEUE_NAME is idle.  enabled since ..."
                //               or: "printer QUEUE_NAME disabled since ..."
                if let Some(rest) = line.strip_prefix("printer ") {
                    // Queue name is the next token
                    if let Some(queue_name) = rest.split_whitespace().next() {
                        let is_default = default_name.as_deref() == Some(queue_name);
                        let is_online = !rest.contains("disabled");
                        // Use queue name as both id and display name — it's what CUPS commands need
                        printers_list.push(PrinterInfo {
                            id: queue_name.to_string(),
                            name: queue_name.replace('_', " "),
                            driver: String::new(),
                            is_default,
                            is_online,
                        });
                    }
                }
            }
        }
    }

    // Fallback to printers crate if lpstat returned nothing
    if printers_list.is_empty() {
        let system_printers = printers::get_printers();
        let default_printer = printers::get_default_printer();
        let default_name = default_printer.as_ref().map(|p| p.name.clone());

        printers_list = system_printers
            .into_iter()
            .map(|p| {
                let is_default = default_name.as_deref() == Some(&p.name);
                PrinterInfo {
                    id: p.name.clone(),
                    name: p.name.clone(),
                    driver: p.driver_name,
                    is_default,
                    is_online: true,
                }
            })
            .collect();
    }

    printers_list
}

// ─── macOS / Linux: parse `lpoptions -p <printer> -l` ───────────────────────

#[cfg(not(target_os = "windows"))]
pub fn get_printer_capabilities(printer_id: &str) -> PrinterCapabilities {
    use std::process::Command;

    let output = Command::new("lpoptions")
        .args(["-p", printer_id, "-l"])
        .output();

    let options = match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            parse_lpoptions(&text)
        }
        _ => vec![],
    };

    // Always append an orientation option (not always in lpoptions output)
    let has_orientation = options.iter().any(|o| o.key.to_lowercase().contains("orientation"));
    let mut options = options;
    if !has_orientation {
        options.push(PrinterOption {
            key: "orientation-requested".to_string(),
            label: "Orientation".to_string(),
            choices: vec![
                PrinterOptionChoice { value: "3".to_string(), label: "Portrait".to_string() },
                PrinterOptionChoice { value: "4".to_string(), label: "Landscape".to_string() },
                PrinterOptionChoice { value: "5".to_string(), label: "Reverse Landscape".to_string() },
                PrinterOptionChoice { value: "6".to_string(), label: "Reverse Portrait".to_string() },
            ],
            default_choice: Some("3".to_string()),
        });
    }

    PrinterCapabilities { options }
}

/// Parse the output of `lpoptions -p <printer> -l`.
/// Each line looks like:
///   KeyName/Display Label: choice1 *defaultChoice choice3
#[cfg(not(target_os = "windows"))]
fn parse_lpoptions(text: &str) -> Vec<PrinterOption> {
    let mut options = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Split on ": " to get key/label part and choices part
        let Some((key_part, choices_part)) = line.split_once(": ") else {
            continue;
        };

        // key_part is "KeyName/Display Label" or just "KeyName"
        let (key, label) = if let Some((k, l)) = key_part.split_once('/') {
            (k.trim().to_string(), l.trim().to_string())
        } else {
            let k = key_part.trim().to_string();
            let l = humanize_option_key(&k);
            (k, l)
        };

        let mut choices = Vec::new();
        let mut default_choice = None;

        for token in choices_part.split_whitespace() {
            if let Some(stripped) = token.strip_prefix('*') {
                default_choice = Some(stripped.to_string());
                choices.push(PrinterOptionChoice {
                    value: stripped.to_string(),
                    label: humanize_choice_value(stripped),
                });
            } else {
                choices.push(PrinterOptionChoice {
                    value: token.to_string(),
                    label: humanize_choice_value(token),
                });
            }
        }

        // Skip options with only one choice (not useful to configure)
        if choices.len() <= 1 && key != "PageSize" {
            continue;
        }

        options.push(PrinterOption {
            key,
            label,
            choices,
            default_choice,
        });
    }

    options
}

// ─── Windows: query via Print Ticket XML / PowerShell ────────────────────────

#[cfg(target_os = "windows")]
pub fn get_printer_capabilities(printer_id: &str) -> PrinterCapabilities {
    use std::process::Command;

    // Use PowerShell to get the PrintCapabilities XML and parse it
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
try {{
    Add-Type -AssemblyName System.Printing
    $server = New-Object System.Printing.LocalPrintServer
    $queue = $server.GetPrintQueue('{printer_id}')
    $stream = $queue.GetPrintCapabilitiesAsXml()
    $reader = New-Object System.IO.StreamReader($stream)
    $xml = [xml]$reader.ReadToEnd()
    $reader.Close()
    $stream.Close()

    $ns = New-Object System.Xml.XmlNamespaceManager($xml.NameTable)
    $ns.AddNamespace('psf', 'http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework')
    $ns.AddNamespace('psk', 'http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords')
    $ns.AddNamespace('xsi', 'http://www.w3.org/2001/XMLSchema-instance')

    $features = $xml.SelectNodes('//psf:Feature', $ns)
    foreach ($feature in $features) {{
        $featureName = $feature.GetAttribute('name')
        if (-not $featureName) {{ continue }}

        # Emit feature header
        Write-Output "FEATURE:$featureName"

        $options = $feature.SelectNodes('psf:Option', $ns)
        $defaultOpt = $null
        foreach ($opt in $options) {{
            $optName = $opt.GetAttribute('name')
            if (-not $optName) {{
                # Try to get the constrained value
                $val = $opt.SelectSingleNode('.//psf:Value', $ns)
                if ($val) {{ $optName = $val.InnerText }}
            }}
            if ($optName) {{
                # Check if this is marked as default
                $constrained = $opt.GetAttribute('constrained')
                Write-Output "  OPTION:$optName"
            }}
        }}
    }}
}} catch {{
    # Fallback: use Get-PrinterProperty
    try {{
        $props = Get-PrinterProperty -PrinterName '{printer_id}' -ErrorAction Stop
        foreach ($prop in $props) {{
            Write-Output "PROP:$($prop.PropertyName)=$($prop.Value)"
        }}
    }} catch {{
        Write-Output "ERROR:$($_.Exception.Message)"
    }}
}}
"#
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output();

    let mut options = match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            parse_windows_capabilities(&text)
        }
        Err(_) => vec![],
    };

    // Always include orientation
    let has_orientation = options.iter().any(|o| {
        o.key.to_lowercase().contains("orientation")
    });
    if !has_orientation {
        options.push(PrinterOption {
            key: "psk:PageOrientation".to_string(),
            label: "Orientation".to_string(),
            choices: vec![
                PrinterOptionChoice { value: "psk:Portrait".to_string(), label: "Portrait".to_string() },
                PrinterOptionChoice { value: "psk:Landscape".to_string(), label: "Landscape".to_string() },
            ],
            default_choice: Some("psk:Portrait".to_string()),
        });
    }

    PrinterCapabilities { options }
}

#[cfg(target_os = "windows")]
fn parse_windows_capabilities(text: &str) -> Vec<PrinterOption> {
    let mut options: Vec<PrinterOption> = Vec::new();
    let mut current_feature: Option<PrinterOption> = None;
    let mut prop_map: HashMap<String, Vec<String>> = HashMap::new();

    for line in text.lines() {
        let line = line.trim();

        if let Some(feature_name) = line.strip_prefix("FEATURE:") {
            // Save previous feature
            if let Some(feat) = current_feature.take() {
                if !feat.choices.is_empty() {
                    options.push(feat);
                }
            }

            let label = humanize_psk_key(feature_name);
            current_feature = Some(PrinterOption {
                key: feature_name.to_string(),
                label,
                choices: Vec::new(),
                default_choice: None,
            });
        } else if let Some(opt_name) = line.strip_prefix("OPTION:") {
            if let Some(ref mut feat) = current_feature {
                let label = humanize_psk_key(opt_name);
                feat.choices.push(PrinterOptionChoice {
                    value: opt_name.to_string(),
                    label,
                });
            }
        } else if let Some(prop_str) = line.strip_prefix("PROP:") {
            // Fallback: Get-PrinterProperty format "Name=Value"
            if let Some((name, value)) = prop_str.split_once('=') {
                prop_map.entry(name.to_string())
                    .or_default()
                    .push(value.to_string());
            }
        }
    }

    // Save last feature
    if let Some(feat) = current_feature.take() {
        if !feat.choices.is_empty() {
            options.push(feat);
        }
    }

    // If we got properties instead of features, convert them
    if options.is_empty() && !prop_map.is_empty() {
        for (name, values) in prop_map {
            let label = humanize_option_key(&name);
            let choices: Vec<PrinterOptionChoice> = values
                .iter()
                .map(|v| PrinterOptionChoice {
                    value: v.clone(),
                    label: v.clone(),
                })
                .collect();
            if !choices.is_empty() {
                options.push(PrinterOption {
                    key: name,
                    label,
                    choices,
                    default_choice: None,
                });
            }
        }
    }

    options
}

/// Strip "psk:" prefix and split PascalCase into words.
fn humanize_psk_key(key: &str) -> String {
    let key = key.strip_prefix("psk:").unwrap_or(key);
    let key = key.strip_prefix("ns0000:").unwrap_or(key);
    split_pascal_case(key)
}

#[allow(dead_code)]
fn humanize_option_key(key: &str) -> String {
    match key.to_lowercase().as_str() {
        "pagesize" => "Paper Size".to_string(),
        "mediatype" => "Media Type".to_string(),
        "inputslot" => "Input Slot".to_string(),
        "colormodel" => "Color Mode".to_string(),
        "cupsprintquality" => "Print Quality".to_string(),
        "duplex" => "Duplex / 2-Sided".to_string(),
        "outputbin" => "Output Bin".to_string(),
        "resolution" => "Resolution".to_string(),
        _ => split_pascal_case(key),
    }
}

#[allow(dead_code)]
fn humanize_choice_value(value: &str) -> String {
    // Strip common prefixes
    let v = value.strip_prefix("psk:").unwrap_or(value);
    let v = v.strip_prefix("ns0000:").unwrap_or(v);

    // Some known CUPS value mappings
    match v {
        "None" => "Off".to_string(),
        "DuplexNoTumble" => "Long Edge".to_string(),
        "DuplexTumble" => "Short Edge".to_string(),
        "any" => "Any / Auto".to_string(),
        "auto" => "Auto".to_string(),
        "main" => "Main Tray".to_string(),
        "rear" => "Rear Feed".to_string(),
        "photo" => "Photo Tray".to_string(),
        "manual" => "Manual Feed".to_string(),
        "disc" => "Disc Tray".to_string(),
        "stationery" => "Plain Paper".to_string(),
        "stationery-coated" => "Coated Paper".to_string(),
        "stationery-letterhead" => "Letterhead".to_string(),
        "stationery-lightweight" => "Lightweight Paper".to_string(),
        "photographic" => "Photo Paper".to_string(),
        "photographic-glossy" => "Photo Paper Glossy".to_string(),
        "photographic-high-gloss" => "Photo Paper High Gloss".to_string(),
        "photographic-semi-gloss" => "Photo Paper Semi-Gloss".to_string(),
        "photographic-matte" => "Photo Paper Matte".to_string(),
        "envelope" => "Envelope".to_string(),
        "Normal" => "Normal".to_string(),
        "High" => "High".to_string(),
        "Draft" => "Draft".to_string(),
        "Gray" => "Grayscale".to_string(),
        "RGB" => "Color (RGB)".to_string(),
        "CMYK" => "Color (CMYK)".to_string(),
        _ => {
            // If it contains a vendor prefix like "com.epson-", clean it up
            if let Some(stripped) = v.strip_prefix("com.epson-") {
                return split_kebab_case(stripped);
            }
            if v.contains('.') && v.contains("Fullbleed") {
                return format!("{} (Borderless)", v.replace(".Fullbleed", ""));
            }
            v.to_string()
        }
    }
}

fn split_pascal_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && ch.is_uppercase() {
            let prev = s.chars().nth(i - 1);
            if prev.map(|c| c.is_lowercase()).unwrap_or(false) {
                result.push(' ');
            }
        }
        result.push(ch);
    }
    result
}

#[allow(dead_code)]
fn split_kebab_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let mut out = first.to_uppercase().to_string();
                    out.extend(chars);
                    out
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
