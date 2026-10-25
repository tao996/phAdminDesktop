/**
 * Split a command line into argv tokens without invoking a shell.
 * Supports single/double quotes and backslash escapes outside single quotes.
 */
export function parseCommandLine(input: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let quote: "'" | '"' | null = null;
  let escaped = false;

  for (const character of input) {
    if (escaped) {
      current += character;
      escaped = false;
      continue;
    }

    if (character === "\\" && quote !== "'") {
      escaped = true;
      continue;
    }

    if (quote) {
      if (character === quote) {
        quote = null;
      } else {
        current += character;
      }
      continue;
    }

    if (character === "'" || character === '"') {
      quote = character;
      continue;
    }

    if (/\s/.test(character)) {
      if (current) {
        tokens.push(current);
        current = "";
      }
      continue;
    }

    current += character;
  }

  if (escaped) current += "\\";
  if (quote) throw new Error("命令中存在未闭合的引号");
  if (current) tokens.push(current);
  return tokens;
}
