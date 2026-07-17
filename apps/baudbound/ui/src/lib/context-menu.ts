export function installContextMenuGuard(target: EventTarget) {
  const options = { capture: true } as const;
  const preventContextMenu = (event: Event) => event.preventDefault();
  target.addEventListener("contextmenu", preventContextMenu, options);
  return () => target.removeEventListener("contextmenu", preventContextMenu, options);
}
