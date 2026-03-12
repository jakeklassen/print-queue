import { useState } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ThemeProvider } from "@/components/theme-provider";
import { AppSidebar, type View } from "@/components/app-sidebar";
import { DashboardPage } from "@/pages/dashboard";
import { PresetsPage } from "@/pages/presets";
import { QueuePage } from "@/pages/queue";
import { SettingsPage } from "@/pages/settings";
import { useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import "./App.css";

const views: Record<View, React.ComponentType> = {
  dashboard: DashboardPage,
  presets: PresetsPage,
  queue: QueuePage,
  settings: SettingsPage,
};

function App() {
  const [activeView, setActiveView] = useState<View>("dashboard");
  const ActivePage = views[activeView];

  useEffect(() => {
    check()
      .then(async (update) => {
        if (!update) return;
        let permitted = await isPermissionGranted();
        if (!permitted) {
          const result = await requestPermission();
          permitted = result === "granted";
        }
        if (permitted) {
          sendNotification({
            title: "PrintQueue Update Available",
            body: `Version ${update.version} is ready to install. Go to Settings to update.`,
          });
        }
      })
      .catch((e) => console.debug("Update check:", e));
  }, []);

  return (
    <ThemeProvider defaultTheme="system">
      <TooltipProvider>
        <div className="flex h-screen overflow-hidden">
          <AppSidebar activeView={activeView} onNavigate={setActiveView} />
          <main className="flex-1 overflow-y-auto p-6">
            <ActivePage />
          </main>
        </div>
      </TooltipProvider>
    </ThemeProvider>
  );
}

export default App;
