import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, FN, isReserved, keycaps, type KeyInput } from "./shortcut";

const press = (code: string, mods: Partial<KeyInput> = {}): KeyInput => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("acceleratorFromEvent", () => {
  it("orders modifiers before the key", () => {
    expect(
      acceleratorFromEvent(
        press("KeyD", { metaKey: true, shiftKey: true, ctrlKey: true }),
      ),
    ).toBe("Control+Shift+Super+KeyD");
    expect(acceleratorFromEvent(press("Space", { altKey: true }))).toBe(
      "Alt+Space",
    );
  });

  it("waits while only modifiers are held", () => {
    expect(acceleratorFromEvent(press("AltLeft", { altKey: true }))).toBeNull();
    expect(
      acceleratorFromEvent(press("ShiftRight", { shiftKey: true })),
    ).toBeNull();
  });

  it("refuses keys that would just type text", () => {
    expect(acceleratorFromEvent(press("KeyA"))).toBeNull();
    expect(acceleratorFromEvent(press("KeyA", { shiftKey: true }))).toBeNull();
    expect(acceleratorFromEvent(press("Space"))).toBeNull();
  });

  it("allows a bare function key", () => {
    expect(acceleratorFromEvent(press("F5"))).toBe("F5");
  });
});

describe("keycaps", () => {
  it("uses symbols on macOS", () => {
    expect(keycaps("Alt+Space", "macos", { space: "Espace" })).toEqual(["⌥", "Espace"]);
    expect(keycaps("Control+Shift+Super+KeyD", "macos")).toEqual([
      "⌃",
      "⇧",
      "⌘",
      "D",
    ]);
  });

  it("uses names elsewhere", () => {
    expect(keycaps("Control+Space", "windows")).toEqual(["Ctrl", "Space"]);
    expect(keycaps("Control+Space", "windows", { space: "Leertaste", ctrl: "Strg" })).toEqual([
      "Strg",
      "Leertaste",
    ]);
    expect(keycaps("Super+Digit1", "windows")).toEqual(["Win", "1"]);
    expect(keycaps("Super+ArrowUp", "linux")).toEqual(["Super", "↑"]);
  });
});

describe("isReserved", () => {
  it("refuses everyday shortcuts", () => {
    expect(isReserved("Super+KeyX", "macos")).toBe(true);
    expect(isReserved("Super+Space", "macos")).toBe(true);
    expect(isReserved("Control+KeyC", "windows")).toBe(true);
  });

  it("allows combinations nothing else uses", () => {
    expect(isReserved("Alt+Space", "macos")).toBe(false);
    expect(isReserved("Super+Shift+KeyX", "macos")).toBe(false);
    expect(isReserved("Control+KeyX", "macos")).toBe(false);
    expect(isReserved("Control+Space", "windows")).toBe(false);
  });
});

describe("the fn key", () => {
  it("is drawn as fn and never reserved", () => {
    expect(keycaps(FN, "macos")).toEqual(["fn"]);
    expect(isReserved(FN, "macos")).toBe(false);
  });
});
