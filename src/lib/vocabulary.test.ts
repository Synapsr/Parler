import { describe, expect, it } from "vitest";
import { parseWords } from "./vocabulary";

describe("parseWords", () => {
  it("splits on commas and lines, trims, and drops duplicates", () => {
    expect(parseWords("Synapsr, Groq\n groq ,, Plougastel")).toEqual([
      "Synapsr",
      "Groq",
      "Plougastel",
    ]);
    expect(parseWords("  ")).toEqual([]);
  });
});
