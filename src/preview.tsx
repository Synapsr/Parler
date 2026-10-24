// Dev-only: renders the windows in a browser with a fake backend.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/instrument-serif/400.css";
import "@fontsource/instrument-serif/400-italic.css";
import "@fontsource-variable/schibsted-grotesk";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";

const params = new URLSearchParams(location.search);
const view = params.get("view") ?? "main";
const onboarded = params.get("onboarded") === "1";
const platform = params.get("platform") ?? "macos";

let settings = {
  onboarded,
  provider: "groq",
  apiKeys: onboarded || params.get("key") ? { groq: "gsk_demo" } : {},
  models: {},
  customBaseUrl: "http://localhost:8000/v1",
  language: "fr",
  vocabulary: "",
  shortcut: platform === "macos" ? "Alt+Space" : "Control+Space",
  shortcutMode: "smart",
  microphone: null,
  output: "paste",
  sounds: true,
  uiLanguage: params.get("lang") ?? "fr",
  theme: params.get("theme") ?? "system",
};
const now = Date.now();
const HOUR = 3_600_000;
const HISTORY = [
  "Bonjour Camille, je te confirme le rendez-vous de jeudi à 14\u00a0h à Plougastel. Je t'envoie le devis dans la journée.",
  "Ajouter un test pour le cas où le micro se déconnecte pendant l'enregistrement.",
  "Merci pour ton retour, je regarde ça demain matin.",
  "Penser à renouveler le nom de domaine avant la fin du mois.",
  "Le rendu est vraiment top, on peut valider la maquette pour la page d'accueil.",
  "Rappelle-moi d'appeler la boulangerie demain à neuf heures.",
  "On décale la réunion d'équipe à mardi, même heure.",
  "Liste de courses : lait, œufs, café, pain complet.",
  "Je serai en télétravail vendredi, joignable par message.",
  "Super idée, on en parle à la prochaine rétro.",
].map((text, i) => ({
  id: now - [40_000, HOUR, 3 * HOUR, 26 * HOUR, 28 * HOUR, 30 * HOUR, 50 * HOUR, 75 * HOUR, 76 * HOUR, 100 * HOUR][i],
  text,
  durationMs: 1800 + ((i * 1371) % 5000),
  provider: "Groq",
}));
const problem = params.get("problem")
  ? { kind: "transcription", error: { kind: "offline", provider: "Groq" } }
  : null;

mockWindows(view === "overlay" ? "overlay" : "main");
mockIPC(
  (cmd, args) => {
    const a = args as Record<string, unknown>;
    switch (cmd) {
      case "app_info":
        return { platform, version: "0.1.0" };
      case "get_settings":
        return settings;
      case "save_settings":
        settings = a.settings as typeof settings;
        return settings;
      case "get_providers":
        return [
          { id: "groq", label: "Groq", baseUrl: "", defaultModel: "whisper-large-v3-turbo", models: ["whisper-large-v3-turbo", "whisper-large-v3"], requiresApiKey: true, apiKeyUrl: "https://console.groq.com/keys" },
          { id: "openai", label: "OpenAI", baseUrl: "", defaultModel: "gpt-4o-mini-transcribe", models: ["gpt-4o-mini-transcribe", "gpt-4o-transcribe", "whisper-1"], requiresApiKey: true, apiKeyUrl: "https://platform.openai.com/api-keys" },
          { id: "fireworks", label: "Fireworks", baseUrl: "", defaultModel: "whisper-v3-turbo", models: ["whisper-v3-turbo", "whisper-v3"], requiresApiKey: true, apiKeyUrl: "https://app.fireworks.ai" },
          { id: "custom", label: "Custom", baseUrl: "", defaultModel: "whisper-1", models: [], requiresApiKey: false, apiKeyUrl: null },
        ];
      case "get_status":
        return {
          state: params.get("state") ?? "idle",
          problem,
          canRetry: Boolean(problem),
        };
      case "get_history":
        return onboarded ? HISTORY : [];
      case "get_stats":
        return onboarded
          ? { dictations: 214, words: 9840, speechMs: 3_420_000 }
          : { dictations: 0, words: 0, speechMs: 0 };
      case "list_microphones":
        return [
          { id: "1", name: "MacBook Pro Microphone", isDefault: true },
          { id: "2", name: "AirPods Pro", isDefault: false },
        ];
      case "test_provider":
        return new Promise((resolve) => setTimeout(() => resolve(null), 700));
      case "plugin:macos-permissions|check_microphone_permission":
        return true;
      case "plugin:macos-permissions|check_accessibility_permission":
        return false;
      case "plugin:autostart|is_enabled":
        return true;
      default:
        return null;
    }
  },
  { shouldMockEvents: true },
);

if (view === "overlay") {
  await import("./overlay/overlay.css");
  document.body.style.background = "#3a3631";
  const { Overlay } = await import("./overlay/Overlay");
  createRoot(document.getElementById("root")!).render(<Overlay />);
  const phase = params.get("phase") ?? "recording";
  setTimeout(async () => {
    await emit("parler://overlay", { phase, sounds: false, problem: phase === "error" ? { kind: "transcription", error: { kind: "unauthorized", provider: "Groq" } } : null });
    let t = 0;
    setInterval(() => {
      t += 1;
      emit("parler://level", Math.abs(Math.sin(t / 3)) * 0.8 * Math.random() + 0.1);
    }, 60);
  }, 300);
} else {
  await import("./styles.css");
  // The size of the real window.
  const root = document.getElementById("root")!;
  root.style.width = "900px";
  root.style.height = `${params.get("h") ?? 700}px`;
  // `clicks=Continuer,Vérifier` presses those buttons in turn.
  const clicks = (params.get("clicks") ?? "").split(",").filter(Boolean);
  clicks.forEach((label, i) =>
    setTimeout(() => {
      const button = [...document.querySelectorAll("button")].find((b) =>
        b.textContent?.includes(label) || b.getAttribute("aria-label") === label,
      );
      button?.click();
    }, 600 + i * 1200),
  );
  const { default: App } = await import("./App");
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
