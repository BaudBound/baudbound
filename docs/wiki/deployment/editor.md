---
title: Editor Deployment
description: Build and host the BaudBound Next.js editor.
tags: [deployment, editor]
---
# Editor Deployment

The production editor is hosted at [editor.baudbound.app](https://editor.baudbound.app/). Its container workflow runs lint, TypeScript, schema freshness, unit, build, and browser checks before publication.

The editor is client-heavy and performs package generation in the browser. Serve it through HTTPS with headers compatible with downloads, browser storage, workers, and the libraries used by Next.js and React Flow. Do not cache mutable HTML indefinitely; immutable hashed assets may use long-lived caching.

The public deployment does not receive runner secrets. Packages are exported to the user's machine and later imported into a runner. Availability monitoring should cover page load, static assets, and a basic editor interaction rather than only TCP reachability.

Use the published image tag matching the intended release and retain rollback access to the previous known-good image.
