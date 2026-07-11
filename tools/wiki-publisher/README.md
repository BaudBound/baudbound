# Wiki.js Publisher

This package validates `docs/wiki/**/*.md` and reconciles those pages through the Wiki.js GraphQL API. Repository Markdown is the source of truth.

## Page contract

Every page requires YAML frontmatter:

```yaml
---
title: Runner Quick Start
description: Import, approve, and execute a BaudBound package.
published: true
private: false
locale: en
tags:
  - runner
---
```

`published`, `private`, `locale`, and `tags` are optional. Internal Markdown links are resolved from source files and rewritten to Wiki.js paths. Local images are rejected until they have an HTTPS asset URL.

The publisher adds the reserved `baudbound-docs` and `managed-by-git` tags. It only deletes pages carrying both tags. Matching unmanaged pages are never overwritten unless a manual workflow run explicitly enables adoption.

## GitHub setup

Create a `wiki-production` GitHub environment with these secrets:

- `WIKI_URL`: the public HTTPS root URL of the Wiki.js installation;
- `WIKI_API_TOKEN`: a Wiki.js API token limited to `read:pages`, `read:source`, `write:pages`, and `delete:pages`.

Initial publication:

1. Run **Wiki Documentation** manually with `dry_run` enabled.
2. If Wiki.js already has a placeholder `home` page, enable `adopt_existing_pages` and inspect the dry-run plan.
3. Run it again with the same adoption setting and `dry_run` disabled.
4. Confirm the managed pages in Wiki.js.
5. Create the repository Actions variable `WIKI_PUBLISH_ENABLED=true`.

After that, pushes affecting public documentation publish automatically. Pull requests only validate content and never receive production secrets.

## Local commands

```bash
pnpm --dir tools/wiki-publisher install
pnpm --dir tools/wiki-publisher test
pnpm --dir tools/wiki-publisher validate
```

Publishing locally requires `WIKI_URL` and `WIKI_API_TOKEN`. Use `WIKI_DRY_RUN=true` before any direct publication.
