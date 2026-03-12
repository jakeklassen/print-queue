import { useEffect, useState } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { getPrinterCapabilities } from "@/lib/api";
import type { PrinterCapabilities, PrintSettings } from "@/lib/types";

export function CapabilitiesForm({
  printerId,
  settings,
  onChange,
}: {
  printerId: string | null;
  settings: PrintSettings;
  onChange: (settings: PrintSettings) => void;
}) {
  const [capabilities, setCapabilities] = useState<PrinterCapabilities | null>(
    null,
  );
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!printerId) {
      setCapabilities(null);
      return;
    }

    setLoading(true);
    getPrinterCapabilities(printerId)
      .then((caps) => {
        setCapabilities(caps);
        // Pre-fill defaults for any option not already set
        const updated = { ...settings };
        let changed = false;

        for (const opt of caps.options) {
          if (!updated[opt.key] && opt.default_choice) {
            updated[opt.key] = opt.default_choice;
            changed = true;
          }
        }

        if (changed) {
          onChange(updated);
        }
      })
      .catch(() => setCapabilities(null))
      .finally(() => setLoading(false));
    // Only re-fetch when printerId changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [printerId]);

  if (!printerId) {
    return (
      <p className="text-sm text-muted-foreground">
        Select a printer to see available settings.
      </p>
    );
  }

  if (loading) {
    return (
      <p className="text-sm text-muted-foreground">Loading capabilities...</p>
    );
  }

  if (!capabilities || capabilities.options.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        Could not load printer capabilities.
      </p>
    );
  }

  return (
    <div className="grid gap-3">
      {capabilities.options.map((option) => (
        <div key={option.key} className="grid gap-1.5">
          <Label className="text-sm flex items-center gap-2">
            {option.label}
            {option.default_choice &&
              settings[option.key] === option.default_choice && (
                <Badge
                  variant="outline"
                  className="text-[10px] px-1 py-0 font-normal"
                >
                  default
                </Badge>
              )}
          </Label>
          <Select
            value={settings[option.key] ?? ""}
            onValueChange={(val) =>
              onChange({ ...settings, [option.key]: val })
            }
          >
            <SelectTrigger>
              <SelectValue
                placeholder={`Select ${option.label.toLowerCase()}...`}
              />
            </SelectTrigger>
            <SelectContent>
              {option.choices.map((choice) => (
                <SelectItem key={choice.value} value={choice.value}>
                  <span className="flex items-center gap-2">
                    {choice.label}
                    {choice.value === option.default_choice && (
                      <span className="text-muted-foreground text-xs">
                        (default)
                      </span>
                    )}
                  </span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      ))}
    </div>
  );
}
