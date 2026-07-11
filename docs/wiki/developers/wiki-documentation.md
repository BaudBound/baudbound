---
title: Wiki Documentation
description: Author, validate, publish, and safely reconcile repository documentation with Wiki.js.
tags: [developers, documentation, wiki]
---
# Wiki Documentation

`docs/wiki/**/*.md` is the source of truth for public documentation. Each page requires `title` and `description` frontmatter; `published`, `private`, `locale`, and `tags` are optional. Internal Markdown links are resolved and rewritten to Wiki.js paths. Local images are rejected; use an HTTPS asset URL.

`docs/wiki/navigation.json` is the source of truth for the static sidebar. Every managed page in its locale must appear exactly once. IDs are stable, links are validated against loaded pages, and the publisher reconciles the tree and `STATIC` navigation mode after page publication.

The publisher adds `baudbound-docs` and `managed-by-git` tags. It only updates or deletes pages under that ownership contract. A matching unmanaged Wiki.js page is not overwritten unless explicit adoption is enabled for a reviewed run.

Validate locally:

```text
pnpm --dir tools/wiki-publisher install
pnpm --dir tools/wiki-publisher test
pnpm --dir tools/wiki-publisher validate
```

GitHub uses the `wiki-production` environment. `WIKI_URL` contains the HTTPS Wiki.js root. `WIKI_API_TOKEN` needs page read, source read, page write, page delete, and navigation management permissions. `WIKI_PUBLISH_ENABLED=true` enables publication after validation.

Pull requests validate without production secrets. Changes on `master` reconcile pages and static navigation automatically. A dry run reports whether navigation would change without mutating Wiki.js. Publication reads navigation before changing pages, so a token without navigation access fails during preflight.
