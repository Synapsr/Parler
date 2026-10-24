// Shortcuts are stored as accelerators the global-shortcut plugin parses,
// e.g. "Control+Shift+KeyD": modifiers first, then one KeyboardEvent.code.
// On macOS, `FN` is the fn (🌐) key alone, which the backend follows itself.

/** The fn (🌐) key alone, macOS only: it can't be recorded like other keys. */
export const FN = "Fn";

const MODIFIERS = ["Control", "Alt", "Shift", "Super"] as const;
type Modifier = (typeof MODIFIERS)[number];

const NAMED_KEYS = new Set([
  "Space",
  "Enter",
  "Tab",
  "Backspace",
  "Delete",
  "Insert",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Backquote",
  "Minus",
  "Equal",
  "BracketLeft",
  "BracketRight",
  "Backslash",
  "Semicolon",
  "Quote",
  "Comma",
  "Period",
  "Slash",
]);

function isSupportedCode(code: string): boolean {
  return (
    NAMED_KEYS.has(code) ||
    /^Key[A-Z]$/.test(code) ||
    /^Digit[0-9]$/.test(code) ||
    /^F([1-9]|1[0-9]|2[0-4])$/.test(code) ||
    /^Numpad[0-9]$/.test(code)
  );
}

const isFunctionKey = (code: string) => /^F\d+$/.test(code);

export type KeyInput = Pick<
  KeyboardEvent,
  "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey"
>;

/**
 * The accelerator for a key press, or null while only modifiers are down or
 * when the combination would hijack ordinary typing (a bare letter).
 */
export function acceleratorFromEvent(event: KeyInput): string | null {
  if (!isSupportedCode(event.code)) return null;
  const held: Record<Modifier, boolean> = {
    Control: event.ctrlKey,
    Alt: event.altKey,
    Shift: event.shiftKey,
    Super: event.metaKey,
  };
  const modifiers = MODIFIERS.filter((m) => held[m]);
  // Shift alone still types: Shift+A is just "A".
  const typesText = modifiers.length === 0 || (modifiers.length === 1 && held.Shift);
  if (typesText && !isFunctionKey(event.code)) return null;
  return [...modifiers, event.code].join("+");
}

const MAC_SYMBOLS: Record<string, string> = {
  Control: "⌃",
  Alt: "⌥",
  Shift: "⇧",
  Super: "⌘",
};

const OTHER_NAMES: Record<string, string> = {
  Control: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
  Super: "Win",
};

const KEY_SYMBOLS: Record<string, string> = {
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  PageUp: "Page ↑",
  PageDown: "Page ↓",
};

/** Key names that change with the interface language (German keyboards say "Strg"). */
export interface KeyNames {
  space?: string;
  ctrl?: string;
}

/** The keycaps to draw for an accelerator, with `names` translating some keys. */
export function keycaps(accelerator: string, platform: string, names: KeyNames = {}): string[] {
  const mac = platform === "macos";
  // As printed on the key; the keycap draws the globe next to it.
  if (accelerator === FN) return ["fn"];
  return accelerator
    .split("+")
    .filter(Boolean)
    .map((token) => {
      if (token in MAC_SYMBOLS) {
        if (mac) return MAC_SYMBOLS[token];
        if (token === "Super" && platform === "linux") return "Super";
        if (token === "Control") return names.ctrl ?? OTHER_NAMES.Control;
        return OTHER_NAMES[token];
      }
      if (token === "Space") return names.space ?? "Space";
      if (token === "Enter") return mac ? "↩" : "Enter";
      if (token === "Backspace") return mac ? "⌫" : "Backspace";
      if (token === "Tab") return mac ? "⇥" : "Tab";
      if (token in KEY_SYMBOLS) return KEY_SYMBOLS[token];
      const letter = /^Key([A-Z])$/.exec(token);
      if (letter) return letter[1];
      const digit = /^(?:Digit|Numpad)([0-9])$/.exec(token);
      if (digit) return digit[1];
      return token;
    });
}

// Everyday shortcuts a dictation key must never take over: with only the
// platform's command modifier held, these keys copy, paste, save, quit...
const EVERYDAY_KEYS = new Set([
  "KeyA",
  "KeyC",
  "KeyF",
  "KeyN",
  "KeyO",
  "KeyP",
  "KeyQ",
  "KeyS",
  "KeyT",
  "KeyV",
  "KeyW",
  "KeyX",
  "KeyY",
  "KeyZ",
  "Tab",
  "Space",
]);

/** Whether `accelerator` would shadow a system or everyday app shortcut. */
export function isReserved(accelerator: string, platform: string): boolean {
  if (accelerator === FN) return false;
  const tokens = accelerator.split("+");
  const key = tokens[tokens.length - 1];
  const modifiers = tokens.slice(0, -1);
  const command = platform === "macos" ? "Super" : "Control";
  // ⌘Space opens Spotlight on macOS; Ctrl+Space stays allowed elsewhere.
  if (key === "Space" && platform !== "macos") return false;
  return modifiers.length === 1 && modifiers[0] === command && EVERYDAY_KEYS.has(key);
}
