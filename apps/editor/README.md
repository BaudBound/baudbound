# BaudBound Editor

Production-focused web app for visually creating scripts and exporting `.bbs` packages.

The editor does not connect to runners and exported packages are still validated by the runner. The in-browser simulator is an editor-only preview tool; it does not make the browser a trusted runtime.

## Checks

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

## Docker

```bash
docker build -t baudbound-editor .
docker run --rm -p 3000:3000 baudbound-editor
```

The editor is browser-local and does not require a backend service.
