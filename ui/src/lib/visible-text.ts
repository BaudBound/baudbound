export function visibleText(value: string) {
  let visible = "";
  for (const character of value) {
    switch (character) {
      case "\\":
        visible += "\\\\";
        break;
      case "\b":
        visible += "\\b";
        break;
      case "\f":
        visible += "\\f";
        break;
      case "\n":
        visible += "\\n";
        break;
      case "\r":
        visible += "\\r";
        break;
      case "\t":
        visible += "\\t";
        break;
      case "\0":
        visible += "\\0";
        break;
      default:
        visible += isControlCharacter(character)
          ? `\\u{${character.codePointAt(0)?.toString(16) ?? "0"}}`
          : character;
    }
  }
  return visible;
}

export function quotedVisibleText(value: string) {
  return JSON.stringify(value);
}

function isControlCharacter(character: string) {
  const codePoint = character.codePointAt(0);
  return codePoint !== undefined && (codePoint < 0x20 || (codePoint >= 0x7f && codePoint <= 0x9f));
}
