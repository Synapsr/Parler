/**
 * The vocabulary is stored as one comma-separated hint, as the model
 * receives it; the interface shows it as distinct words.
 */
export function parseWords(vocabulary: string): string[] {
  const seen = new Set<string>();
  return vocabulary
    .split(/[,\n]/)
    .map((word) => word.trim())
    .filter((word) => {
      const key = word.toLowerCase();
      if (!word || seen.has(key)) return false;
      seen.add(key);
      return true;
    });
}
