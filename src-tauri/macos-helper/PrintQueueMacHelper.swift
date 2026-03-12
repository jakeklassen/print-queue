import AppKit
import ApplicationServices
import Foundation
import PDFKit

struct PrinterRecord: Codable {
    let id: String
    let name: String
    let isDefault: Bool

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case isDefault = "is_default"
    }
}

struct ConfigureResponse: Codable {
    let printerName: String
    let printInfoBase64: String
    let pageFormatBase64: String
    let printSettingsBase64: String
    let pageWidthPoints: Double
    let pageHeightPoints: Double

    enum CodingKeys: String, CodingKey {
        case printerName = "printer_name"
        case printInfoBase64 = "print_info_base64"
        case pageFormatBase64 = "page_format_base64"
        case printSettingsBase64 = "print_settings_base64"
        case pageWidthPoints = "page_width_points"
        case pageHeightPoints = "page_height_points"
    }
}

enum HelperError: Error, CustomStringConvertible {
    case message(String)

    var description: String {
        switch self {
        case .message(let value):
            return value
        }
    }
}

func buildPrintInfo(printerName: String?, paperSize: NSSize?) -> NSPrintInfo {
    let printInfo = NSPrintInfo.shared.copy() as! NSPrintInfo
    printInfo.jobDisposition = .spool
    printInfo.isHorizontallyCentered = false
    printInfo.isVerticallyCentered = false
    printInfo.horizontalPagination = .clip
    printInfo.verticalPagination = .clip
    printInfo.topMargin = 0
    printInfo.bottomMargin = 0
    printInfo.leftMargin = 0
    printInfo.rightMargin = 0
    printInfo.dictionary()[NSPrintInfo.AttributeKey.detailedErrorReporting] = NSNumber(value: true)
    if let paperSize {
        printInfo.paperSize = paperSize
    }

    if let printerName, let printer = NSPrinter(name: printerName) {
        printInfo.printer = printer
        printInfo.dictionary()[NSPrintInfo.AttributeKey.printerName] = printer.name
    }

    return printInfo
}

func archivePrintInfo(_ printInfo: NSPrintInfo) throws -> String {
    let data = try NSKeyedArchiver.archivedData(withRootObject: printInfo, requiringSecureCoding: false)
    return data.base64EncodedString()
}

func archivePMPageFormat(_ printInfo: NSPrintInfo) throws -> String {
    let pmPageFormat = unsafeBitCast(printInfo.pmPageFormat(), to: PMPageFormat.self)
    var cfData: Unmanaged<CFData>?
    let status = PMPageFormatCreateDataRepresentation(pmPageFormat, &cfData, kPMDataFormatXMLDefault)
    guard status == noErr, let data = cfData?.takeRetainedValue() as Data? else {
        throw HelperError.message("Could not serialize PMPageFormat (status \(status))")
    }
    return data.base64EncodedString()
}

func archivePMPrintSettings(_ printInfo: NSPrintInfo) throws -> String {
    let pmPrintSettings = unsafeBitCast(printInfo.pmPrintSettings(), to: PMPrintSettings.self)
    var cfData: Unmanaged<CFData>?
    let status = PMPrintSettingsCreateDataRepresentation(pmPrintSettings, &cfData, kPMDataFormatXMLDefault)
    guard status == noErr, let data = cfData?.takeRetainedValue() as Data? else {
        throw HelperError.message("Could not serialize PMPrintSettings (status \(status))")
    }
    return data.base64EncodedString()
}

func restorePrintInfo(from base64: String) throws -> NSPrintInfo {
    guard let data = Data(base64Encoded: base64) else {
        throw HelperError.message("Invalid print info payload")
    }
    guard let object = try NSKeyedUnarchiver.unarchiveTopLevelObjectWithData(data) as? NSPrintInfo else {
        throw HelperError.message("Could not decode macOS print settings")
    }
    return object
}

func emitJSON<T: Encodable>(_ value: T) throws {
    let encoder = JSONEncoder()
    let data = try encoder.encode(value)
    guard let text = String(data: data, encoding: .utf8) else {
        throw HelperError.message("Could not encode JSON output")
    }
    FileHandle.standardOutput.write(Data(text.utf8))
}

func logPrintSettings(_ label: String, printInfo: NSPrintInfo) {
    let pairs = printInfo.printSettings.allKeys
        .map { String(describing: $0) }
        .sorted()
        .joined(separator: ", ")
    fputs("[PrintQueue][macOS] \(label) printSettings keys: \(pairs)\n", stderr)
}

func applyPMState(
    printInfo: NSPrintInfo,
    pageFormatBase64: String?,
    printSettingsBase64: String?
) throws {
    if let pageFormatBase64, let data = Data(base64Encoded: pageFormatBase64) {
        var restoredPageFormat: PMPageFormat?
        let createStatus = PMPageFormatCreateWithDataRepresentation(data as CFData, &restoredPageFormat)
        guard createStatus == noErr, let restoredPageFormat else {
            throw HelperError.message("Could not restore PMPageFormat (status \(createStatus))")
        }
        let targetPageFormat = unsafeBitCast(printInfo.pmPageFormat(), to: PMPageFormat.self)
        let copyStatus = PMCopyPageFormat(restoredPageFormat, targetPageFormat)
        guard copyStatus == noErr else {
            throw HelperError.message("Could not copy PMPageFormat (status \(copyStatus))")
        }
        printInfo.updateFromPMPageFormat()
    }

    if let printSettingsBase64, let data = Data(base64Encoded: printSettingsBase64) {
        var restoredPrintSettings: PMPrintSettings?
        let createStatus = PMPrintSettingsCreateWithDataRepresentation(data as CFData, &restoredPrintSettings)
        guard createStatus == noErr, let restoredPrintSettings else {
            throw HelperError.message("Could not restore PMPrintSettings (status \(createStatus))")
        }
        let targetPrintSettings = unsafeBitCast(printInfo.pmPrintSettings(), to: PMPrintSettings.self)
        let copyStatus = PMCopyPrintSettings(restoredPrintSettings, targetPrintSettings)
        guard copyStatus == noErr else {
            throw HelperError.message("Could not copy PMPrintSettings (status \(copyStatus))")
        }
        printInfo.updateFromPMPrintSettings()
    }
}

func listPrinters() throws {
    let defaultName = NSPrintInfo.defaultPrinter?.name
    let records = NSPrinter.printerNames.sorted().map { name in
        PrinterRecord(id: name, name: name, isDefault: name == defaultName)
    }
    try emitJSON(records)
}

func configure(arguments: [String], printerHint: String?) throws {
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    app.activate(ignoringOtherApps: true)

    let paperWidth = Double(parseFlag("--paper-width", in: arguments) ?? "")
    let paperHeight = Double(parseFlag("--paper-height", in: arguments) ?? "")
    let explicitPaperSize: NSSize? = {
        guard let paperWidth, let paperHeight, paperWidth > 0, paperHeight > 0 else {
            return nil
        }
        return NSSize(width: paperWidth, height: paperHeight)
    }()

    let printInfo = buildPrintInfo(printerName: printerHint, paperSize: explicitPaperSize)
    let pageLayout = NSPageLayout()
    let pageLayoutResult = pageLayout.runModal(with: printInfo)
    if pageLayoutResult != NSApplication.ModalResponse.OK.rawValue {
        throw HelperError.message("User cancelled macOS page layout")
    }

    let printPanel = NSPrintPanel()
    printPanel.options = [
        .showsCopies,
        .showsPaperSize,
        .showsOrientation,
        .showsScaling,
        .showsPageSetupAccessory
    ]
    printPanel.setDefaultButtonTitle("Save")
    printPanel.jobStyleHint = .photo

    let printPanelResult = printPanel.runModal(with: printInfo)
    if printPanelResult != NSApplication.ModalResponse.OK.rawValue {
        throw HelperError.message("User cancelled macOS printer configuration")
    }

    let configuredPrintInfo = printInfo
    logPrintSettings("captured", printInfo: configuredPrintInfo)
    let printerName = configuredPrintInfo.printer.name.isEmpty
        ? ((configuredPrintInfo.dictionary()[NSPrintInfo.AttributeKey.printerName] as? String) ?? printerHint ?? "")
        : configuredPrintInfo.printer.name

    if printerName.isEmpty {
        throw HelperError.message("No printer was selected in the macOS print dialog")
    }

    let printInfoBase64: String
    do {
        printInfoBase64 = try archivePrintInfo(configuredPrintInfo)
    } catch {
        throw HelperError.message("Failed to archive NSPrintInfo: \(error)")
    }

    let pageFormatBase64: String
    do {
        pageFormatBase64 = try archivePMPageFormat(configuredPrintInfo)
    } catch {
        throw HelperError.message("Failed to archive PMPageFormat: \(error)")
    }

    let printSettingsBase64: String
    do {
        printSettingsBase64 = try archivePMPrintSettings(configuredPrintInfo)
    } catch {
        throw HelperError.message("Failed to archive PMPrintSettings: \(error)")
    }

    fputs(
        "[PrintQueue][macOS] captured blob lengths: printInfo=\(printInfoBase64.count), pageFormat=\(pageFormatBase64.count), printSettings=\(printSettingsBase64.count)\n",
        stderr
    )

    try emitJSON(
        ConfigureResponse(
            printerName: printerName,
            printInfoBase64: printInfoBase64,
            pageFormatBase64: pageFormatBase64,
            printSettingsBase64: printSettingsBase64,
            pageWidthPoints: Double(configuredPrintInfo.paperSize.width),
            pageHeightPoints: Double(configuredPrintInfo.paperSize.height)
        )
    )
}

func printPDF(
    filePath: String,
    copies: Int,
    printInfoBase64: String,
    pageFormatBase64: String?,
    printSettingsBase64: String?,
    printerName: String?
) throws {
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)

    guard let document = PDFDocument(url: URL(fileURLWithPath: filePath)) else {
        throw HelperError.message("Could not load PDF at \(filePath)")
    }

    let printInfo = try restorePrintInfo(from: printInfoBase64)
    try applyPMState(
        printInfo: printInfo,
        pageFormatBase64: pageFormatBase64,
        printSettingsBase64: printSettingsBase64
    )
    printInfo.jobDisposition = .spool
    printInfo.dictionary()[NSPrintInfo.AttributeKey.copies] = NSNumber(value: max(1, copies))
    printInfo.dictionary()[NSPrintInfo.AttributeKey.detailedErrorReporting] = NSNumber(value: true)

    if let printerName, let printer = NSPrinter(name: printerName) {
        printInfo.printer = printer
        printInfo.dictionary()[NSPrintInfo.AttributeKey.printerName] = printer.name
    }

    logPrintSettings("restored-pdf", printInfo: printInfo)

    guard let operation = document.printOperation(for: printInfo, scalingMode: PDFPrintScalingMode.pageScaleNone, autoRotate: false) else {
        throw HelperError.message("Could not create native PDF print operation")
    }
    operation.showsPrintPanel = false
    operation.showsProgressPanel = false

    guard operation.run() else {
        throw HelperError.message("macOS PDF print operation failed")
    }
}

func parseFlag(_ name: String, in arguments: [String]) -> String? {
    guard let index = arguments.firstIndex(of: name), arguments.count > index + 1 else {
        return nil
    }
    return arguments[index + 1]
}

do {
    let arguments = Array(CommandLine.arguments.dropFirst())
    guard let command = arguments.first else {
        throw HelperError.message("Missing helper command")
    }

    switch command {
    case "list-printers":
        try listPrinters()
    case "configure":
        try configure(arguments: arguments, printerHint: parseFlag("--printer", in: arguments))
    case "print":
        guard let filePath = parseFlag("--file", in: arguments) else {
            throw HelperError.message("Missing --file")
        }
        guard let printInfoBase64 = parseFlag("--print-info-b64", in: arguments) else {
            throw HelperError.message("Missing --print-info-b64")
        }
        let copies = Int(parseFlag("--copies", in: arguments) ?? "1") ?? 1
        let pageFormatBase64 = parseFlag("--page-format-b64", in: arguments)
        let printSettingsBase64 = parseFlag("--print-settings-b64", in: arguments)
        if !filePath.lowercased().hasSuffix(".pdf") {
            throw HelperError.message("macOS helper expects a wrapped PDF print file")
        }
        try printPDF(
            filePath: filePath,
            copies: copies,
            printInfoBase64: printInfoBase64,
            pageFormatBase64: pageFormatBase64,
            printSettingsBase64: printSettingsBase64,
            printerName: parseFlag("--printer", in: arguments)
        )
    default:
        throw HelperError.message("Unknown helper command: \(command)")
    }
} catch {
    let message: String
    if let helperError = error as? HelperError {
        message = helperError.description
    } else {
        message = error.localizedDescription
    }
    FileHandle.standardError.write(Data(message.utf8))
    FileHandle.standardError.write(Data("\n".utf8))
    exit(1)
}
