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
// Parler Cloud: `cloud=signed-out|trial|active|canceling|past-due|inactive|canceled|offline`
// (`signed-in` is `active`) picks it as the service, in that state.
// `signin=wait|approve|deny|expire` decides how a sign-in ends (default: approve).
const cloudMode = params.get("cloud");
const signInMode = params.get("signin") ?? "approve";

let settings: Record<string, unknown> & { apiKeys: Record<string, string>; provider: string } = {
  onboarded,
  // A first launch suggests Parler Cloud; an existing setup keeps Groq here.
  provider: cloudMode || !onboarded ? "cloud" : "groq",
  apiKeys: {
    ...(onboarded || params.get("key") ? { groq: "gsk_demo" } : {}),
    ...(cloudMode && cloudMode !== "signed-out" ? { cloud: "pc_demo" } : {}),
  },
  providerBeforeCloud: cloudMode ? "groq" : null,
  models: {},
  customBaseUrl: "http://localhost:8000/v1",
  language: params.get("lang") ?? "fr",
  vocabulary: "",
  // `shortcut=Fn` shows the fn key (macOS).
  shortcut: params.get("shortcut") ?? (platform === "macos" ? "Alt+Space" : "Control+Space"),
  shortcutMode: "smart",
  microphone: null,
  output: "paste",
  sounds: true,
  uiLanguage: params.get("lang") ?? "fr",
  theme: params.get("theme") ?? "system",
};
const now = Date.now();
const HOUR = 3_600_000;
// Sample dictations in the interface language, for screenshots. French has the
// longest list; the others show a handful.
const SAMPLES: Record<string, string[]> = {
  en: [
    "Hi Camille, confirming our meeting on Thursday at 2 pm. I'll send you the quote today.",
    "Add a test for when the microphone disconnects during recording.",
    "Thanks for your feedback, I'll look at it tomorrow morning.",
    "Remember to renew the domain name before the end of the month.",
    "The design looks great, we can sign off on the homepage mockup.",
  ],
  es: [
    "Hola Camille, te confirmo la reunión del jueves a las 14:00. Te envío el presupuesto hoy mismo.",
    "Añadir una prueba para cuando el micrófono se desconecta durante la grabación.",
    "Gracias por tus comentarios, lo miro mañana por la mañana.",
    "Acordarme de renovar el dominio antes de fin de mes.",
    "El diseño queda genial, podemos aprobar la maqueta de la página de inicio.",
  ],
  de: [
    "Hallo Camille, ich bestätige unseren Termin am Donnerstag um 14 Uhr. Das Angebot schicke ich dir heute noch.",
    "Einen Test für den Fall hinzufügen, dass sich das Mikrofon während der Aufnahme trennt.",
    "Danke für dein Feedback, ich schaue es mir morgen früh an.",
    "Daran denken, die Domain vor Monatsende zu verlängern.",
    "Das Design sieht super aus, wir können den Entwurf der Startseite freigeben.",
  ],
  it: [
    "Ciao Camille, ti confermo l'appuntamento di giovedì alle 14. Ti mando il preventivo in giornata.",
    "Aggiungere un test per quando il microfono si disconnette durante la registrazione.",
    "Grazie per il feedback, ci guardo domani mattina.",
    "Ricordarsi di rinnovare il dominio prima della fine del mese.",
    "Il design è davvero ottimo, possiamo approvare il mockup della home page.",
  ],
  pt: [
    "Oi Camille, confirmo nossa reunião de quinta às 14h. Te envio o orçamento ainda hoje.",
    "Adicionar um teste para quando o microfone desconecta durante a gravação.",
    "Obrigado pelo retorno, vejo isso amanhã de manhã.",
    "Lembrar de renovar o domínio antes do fim do mês.",
    "O design ficou ótimo, podemos aprovar o layout da página inicial.",
  ],
  nl: [
    "Hoi Camille, ik bevestig onze afspraak van donderdag om 14.00 uur. Ik stuur je de offerte vandaag nog.",
    "Een test toevoegen voor als de microfoon tijdens de opname wegvalt.",
    "Bedankt voor je feedback, ik kijk er morgenochtend naar.",
    "Niet vergeten de domeinnaam voor het eind van de maand te verlengen.",
    "Het ontwerp ziet er top uit, we kunnen de mockup van de homepage goedkeuren.",
  ],
  pl: [
    "Cześć Camille, potwierdzam nasze spotkanie w czwartek o 14:00. Wyślę Ci dziś wycenę.",
    "Dodać test na wypadek, gdy mikrofon rozłączy się podczas nagrywania.",
    "Dzięki za opinię, przyjrzę się temu jutro rano.",
    "Pamiętać o odnowieniu domeny przed końcem miesiąca.",
    "Projekt wygląda świetnie, możemy zatwierdzić makietę strony głównej.",
  ],
  tr: [
    "Merhaba Camille, perşembe saat 14:00'teki toplantımızı onaylıyorum. Teklifi bugün gönderiyorum.",
    "Kayıt sırasında mikrofonun bağlantısı kesilirse diye bir test ekle.",
    "Geri bildirimin için teşekkürler, yarın sabah bakacağım.",
    "Ay sonundan önce alan adını yenilemeyi unutma.",
    "Tasarım harika görünüyor, ana sayfa taslağını onaylayabiliriz.",
  ],
  ja: [
    "カミーユさん、木曜日14時の打ち合わせ、承知しました。見積もりは本日中にお送りします。",
    "録音中にマイクが切断された場合のテストを追加する。",
    "フィードバックありがとうございます。明日の朝に確認します。",
    "月末までにドメインを更新するのを忘れないように。",
    "デザインとてもいいですね。トップページのモックアップはこれで確定しましょう。",
  ],
  zh: [
    "Camille 你好，确认周四下午两点的会议。报价单我今天发给你。",
    "添加一个测试：录音过程中麦克风断开的情况。",
    "谢谢你的反馈，我明天上午看一下。",
    "记得在月底前续费域名。",
    "设计效果很棒，首页的样稿可以定下来了。",
  ],
  ko: [
    "카미유 님, 목요일 오후 2시 회의 확인했습니다. 견적서는 오늘 중으로 보내 드릴게요.",
    "녹음 중에 마이크 연결이 끊기는 경우에 대한 테스트 추가하기.",
    "피드백 고마워요. 내일 아침에 확인할게요.",
    "월말 전에 도메인 갱신하는 거 잊지 말기.",
    "디자인 정말 좋네요. 홈페이지 시안은 이대로 확정해도 될 것 같아요.",
  ],
};
const FRENCH = [
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
];
const HISTORY = (SAMPLES[params.get("lang") ?? "fr"] ?? FRENCH).map((text, i) => ({
  id: now - [40_000, HOUR, 3 * HOUR, 26 * HOUR, 28 * HOUR, 30 * HOUR, 50 * HOUR, 75 * HOUR, 76 * HOUR, 100 * HOUR][i],
  text,
  durationMs: 1800 + ((i * 1371) % 5000),
  provider: "Groq",
}));
const DAY = 86_400_000;
const at = (days: number) => new Date(now + days * DAY).toISOString();
const usage = (monthHours: number, dayHours = 0.4) => ({
  daySeconds: dayHours * 3600,
  monthSeconds: monthHours * 3600,
  dayLimitSeconds: 14_400,
  monthLimitSeconds: 144_000,
});
const MANAGE_URL = "https://cloud.parler.synapsr.io/account";
const account = (mode: string | null) => {
  const base = {
    email: params.get("email") ?? "camille@example.com",
    status: "active",
    plan: "monthly",
    trialEndsAt: null as string | null,
    renewsAt: at(18) as string | null,
    cancelAtPeriodEnd: false,
    usage: usage(12),
    manageUrl: MANAGE_URL,
  };
  switch (mode) {
    case "active":
    case "signed-in":
      return base;
    case "canceling":
      return { ...base, plan: "yearly", renewsAt: at(140), cancelAtPeriodEnd: true, usage: usage(36.5) };
    case "past-due":
      return { ...base, status: "past_due", usage: usage(21) };
    case "inactive":
      return { ...base, status: "none", plan: null, renewsAt: null, usage: usage(0, 0) };
    case "canceled":
      return { ...base, status: "canceled", renewsAt: null, usage: usage(0, 0) };
    default:
      return { ...base, status: "trialing", trialEndsAt: at(2.4), renewsAt: at(2.4), usage: usage(3.2) };
  }
};
const CLOUD_ERRORS: Record<string, object> = {
  subscription: { kind: "subscriptionRequired", provider: "Parler Cloud", manageUrl: MANAGE_URL },
  fairday: { kind: "fairUseDay", provider: "Parler Cloud" },
  fairmonth: { kind: "fairUseMonth", provider: "Parler Cloud" },
  signedout: { kind: "signedOut", provider: "Parler Cloud" },
};
// `problem=1` (offline), or a Parler Cloud one: `subscription`, `fairday`, `fairmonth`, `signedout`.
const problemParam = params.get("problem");
const problem = problemParam
  ? {
      kind: "transcription",
      error: CLOUD_ERRORS[problemParam] ?? { kind: "offline", provider: "Groq" },
    }
  : null;
let polls = 0;
const settingsChanged = () => setTimeout(() => emit("parler://settings", settings), 0);

mockWindows(view === "overlay" ? "overlay" : "main");
mockIPC(
  (cmd, args) => {
    const a = args as Record<string, unknown>;
    switch (cmd) {
      case "app_info":
        return { platform, version: "0.1.0" };
      case "get_settings":
        return settings;
      case "save_settings": {
        // As in the backend, the window never writes the Parler Cloud token.
        const next = a.settings as typeof settings;
        const apiKeys = { ...next.apiKeys };
        delete apiKeys.cloud;
        if (settings.apiKeys.cloud) apiKeys.cloud = settings.apiKeys.cloud;
        settings = { ...next, apiKeys };
        return settings;
      }
      case "cloud_sign_in_start":
        polls = 0;
        return new Promise((resolve) =>
          setTimeout(
            () =>
              resolve({
                deviceCode: "dc_preview",
                userCode: "K7PM-Q2XW",
                verificationUrl: "https://cloud.parler.synapsr.io/link?code=K7PM-Q2XW",
                expiresIn: 600,
                interval: 2,
                deviceName: "MacBook Pro",
              }),
            500,
          ),
        );
      case "cloud_sign_in_poll":
        polls += 1;
        if (signInMode === "wait" || polls < 2) return { status: "pending" };
        if (signInMode === "deny") return { status: "denied" };
        if (signInMode === "expire") return { status: "expired" };
        settings = { ...settings, provider: "cloud", apiKeys: { ...settings.apiKeys, cloud: "pc_demo" } };
        settingsChanged();
        return { status: "approved", account: account(cloudMode === "signed-out" ? null : cloudMode) };
      case "cloud_account":
        if (!settings.apiKeys.cloud) return null;
        if (cloudMode === "offline") {
          return new Promise((_, reject) =>
            setTimeout(() => reject({ kind: "offline", provider: "Parler Cloud" }), 400),
          );
        }
        return new Promise((resolve) =>
          setTimeout(() => resolve(account(cloudMode === "signed-out" ? null : cloudMode)), 250),
        );
      case "cloud_sign_out": {
        const apiKeys = { ...settings.apiKeys };
        delete apiKeys.cloud;
        settings = { ...settings, apiKeys, provider: (settings.providerBeforeCloud as string) ?? "groq" };
        settingsChanged();
        return settings;
      }
      case "get_providers":
        return [
          { id: "cloud", label: "Parler Cloud", baseUrl: "https://cloud.parler.synapsr.io/v1", defaultModel: "whisper-large-v3-turbo", models: [], requiresApiKey: true, apiKeyUrl: null },
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
      case "globe_key":
        // Never changed in System Settings: macOS still acts on 🌐.
        return { action: null };
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
    // `error=subscription|fairday|fairmonth|signedout` shows a Parler Cloud problem.
    const error = CLOUD_ERRORS[params.get("error") ?? ""] ?? { kind: "unauthorized", provider: "Groq" };
    await emit("parler://overlay", { phase, sounds: false, problem: phase === "error" ? { kind: "transcription", error } : null });
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
