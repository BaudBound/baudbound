export function visibleText(value: string) {
  const quoted = JSON.stringify(value);
  return quoted.slice(1, -1);
}

export function quotedVisibleText(value: string) {
  return JSON.stringify(value);
}
