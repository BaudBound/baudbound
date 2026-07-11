---
title: Wiki Documentation
description: Author, validate, review, publish, and safely reconcile BaudBound documentation with Wiki.js.
tags: [developers, documentation, wiki]
---
# Wiki Documentation

The repository is the source of truth for the public BaudBound wiki. Contributors edit Markdown under `docs/wiki/`; the publisher validates it, converts repository links to Wiki.js paths, reconciles managed pages, and updates the static navigation.

Do not make an important documentation fix only in the Wiki.js editor. A later repository publication will replace managed page content with the committed source.

## Write for the reader

Assume the reader has never used BaudBound and may not know the operating-system tool being discussed. Introduce BaudBound terms before depending on them and link to [Concepts and Glossary](../concepts.md) instead of repeating partial definitions.

Every procedure must state:

1. what the procedure accomplishes;
2. when the reader needs it and when they do not;
3. prerequisites and supported platforms;
4. where commands are run;
5. which placeholders must be replaced;
6. how to choose between alternatives;
7. the expected successful result;
8. what to inspect when a step fails; and
9. the next relevant page.

Use short sentences and one action per numbered step. Put commands in fenced code blocks. Prefer familiar file commands such as `mkdir`, `cp`, `mv`, `chmod`, and `chown` in beginner procedures. Do not hide decisions inside shell conditionals merely to shorten instructions.

Explain destructive consequences before a command that deletes, overwrites, resets, revokes, or exposes data. Follow the command with a verification step.

## Placeholder convention

Use uppercase placeholders consistently:

| Placeholder | Meaning | Example replacement |
| --- | --- | --- |
| `SCRIPT` | Installed script name or ID | `hello-baudbound` |
| `PACKAGE` | Path to a `.bbs` package | `C:\Users\Alice\Downloads\hello.bbs` |
| `TRIGGER` | Trigger node ID | `n-mr3zyt6f-1` |
| `SECRET_NAME` | Declared secret name | `api_token` |
| `DEVICE_ID` | Logical serial device ID | `workbench-scale` |
| `PATH` | Context-specific filesystem path | `~/.local/opt/baudbound/BaudBound.AppImage` |

State that the placeholder must be replaced before showing a command. Do not wrap a placeholder in angle brackets because shells can interpret `<` and `>` as redirection.

When a literal value is required by the product, use lowercase or a realistic value rather than the placeholder convention. For example, a tutorial can intentionally use the webhook hook name `tutorial`.

## Page files and frontmatter

Public pages live under `docs/wiki/**/*.md`. Directory `index.md` files publish at the directory path:

| Source file | Wiki.js path |
| --- | --- |
| `docs/wiki/home.md` | `/home` |
| `docs/wiki/getting-started/index.md` | `/getting-started` |
| `docs/wiki/runner/installation.md` | `/runner/installation` |

Each page requires YAML frontmatter:

```yaml
---
title: Page title
description: One sentence used for page metadata and discovery.
tags: [runner, reference]
---
```

Supported optional fields are `published`, `private`, `locale`, and `tags`. Unknown fields fail validation. Use kebab-case filenames and paths. Avoid renaming a published path without a migration reason because existing bookmarks and external links will break.

## Internal and external links

Write internal links to repository Markdown files:

```markdown
[Installation and Updates](../runner/installation.md)
[Configuration](configuration.md#serial-devices)
```

The publisher verifies the target page and rewrites it to the managed Wiki.js path. Root-relative Wiki.js links are also accepted when they point to a managed page, but source-relative Markdown links are easier to review during file moves.

External web links must use HTTPS; `mailto:` and `tel:` are accepted only for normal links, not images. Link to primary documentation for platform behavior, formats, and dependencies. Do not use a search-results page as a source.

The publisher currently verifies that external links have valid syntax, but it does not guarantee that every remote server is reachable. Review important external links manually.

## Wiki.js components

Use a component when it makes a decision or warning easier to understand. Do not add components only to decorate a page.

### Tabsets

Use tabsets for mutually exclusive paths such as Windows and Linux, or systemd and OpenRC. Keep shared steps outside the tabs.

```markdown
## Choose a platform {.tabset}

### Windows

Windows instructions.

### Linux

Linux instructions.
```

Do not use tabs when readers need to compare both values at once; use a table instead.

### Callouts

Use an information callout for context that affects a decision:

```markdown
An AppImage is a portable executable and is not installed by a package manager.
{.is-info}
```

Use a warning callout for data loss, secret exposure, network exposure, stale approval, or an operation that can affect another application:

```markdown
Binding to `0.0.0.0` can expose the listener to other machines.
{.is-warning}
```

The warning must explain the consequence and the safer next action.

### Tables, code, and diagrams

Use tables for field references, comparisons, support matrices, and compact definitions. Keep procedural steps in numbered lists.

Add a language to fenced code blocks when syntax highlighting helps, such as `toml`, `json`, `yaml`, `powershell`, `bash`, `rust`, or `typescript`. Use `text` for output and commands intended to be identical across supported shells.

Mermaid diagrams may be used only after the production Wiki.js renderer has been tested with the required syntax. Every diagram needs a text explanation so its information remains accessible and useful outside the rendered wiki.

### Images

Store local documentation images only under `docs/wiki/assets/`. Supported formats are PNG, JPEG, and WebP, with a maximum size of 2 MiB per file. SVG is intentionally rejected because active or externally referenced content is harder to audit safely.

Reference an asset relative to the page:

```markdown
![Visual editor regions](../assets/editor/workspace.png)
```

The publisher verifies the path, type, and size, then rewrites it to the repository-controlled HTTPS asset base. `WIKI_ASSET_BASE_URL` may override that base for a reviewed deployment. Missing, oversized, unsupported, insecure, or out-of-directory images fail validation.

Use screenshots for spatial orientation rather than as the only record of labels, commands, or settings. Crop usernames, paths, package IDs, network data, and secrets. Add useful alternative text. The contributor who changes the pictured UI owns updating or removing its screenshot.

## Static navigation

`docs/wiki/navigation.json` is the source of truth for the Wiki.js sidebar. Every managed page in its locale must appear exactly once.

Each item has a stable kebab-case ID. Link items also require a label, managed page path, and Material Design icon beginning with `mdi-`. Headers group related pages but do not replace a landing page.

The publisher rejects:

- duplicate navigation IDs;
- missing or duplicate page links;
- unknown item fields;
- invalid icons or item kinds; and
- managed pages omitted from navigation.

Changing an ID can cause unnecessary Wiki.js navigation replacement. Keep the existing ID when only a label or target title changes.

## Publisher ownership and safety

The publisher adds `baudbound-docs` and `managed-by-git` tags. It updates or deletes only pages under that ownership contract.

A matching unmanaged Wiki.js page is not overwritten unless explicit adoption is enabled for a reviewed run. Managed-page deletion has a safety limit; a larger reconciliation requires an explicit mass-delete option. The publisher reads remote navigation during preflight and updates page content before reconciling the static tree.

Dry-run publication calculates page and navigation changes without mutating Wiki.js. Use it when adopting pages, changing many paths, or reviewing a potentially destructive reconciliation.

## Validate locally

Install the pinned publisher dependencies once:

```text
pnpm --dir tools/wiki-publisher install --frozen-lockfile
```

Run the publisher safety contracts and full documentation validation:

```text
pnpm --dir tools/wiki-publisher test
pnpm --dir tools/wiki-publisher validate
```

Validation checks metadata, page paths, internal links, HTTPS policy, assets, navigation completeness, and publisher contracts. It also compares current source against `docs/wiki/coverage.json`: registered nodes, desktop tab IDs, Clap commands/options, runner config fields, required pages, crates, tools, schemas, and workflows must remain documented. It reports the source file and line for content errors where available.

Do not solve a coverage failure by deleting a source from the manifest. Add or correct the public documentation, or update the baseline only when a reviewed product surface was intentionally removed.

## Add a page

1. Choose the existing section that owns the subject. Create a new page only when the content has a distinct audience or task.
2. Add the Markdown file with required frontmatter and a clear first paragraph.
3. Add the page exactly once to `navigation.json` with a stable ID and relevant icon.
4. Link it from the pages readers are likely to visit before and after it.
5. Run publisher tests and validation.
6. Inspect the rendered page on a wide and narrow viewport after publication.

## Rename or move a page

1. Search for every inbound repository link and published reference.
2. Decide whether breaking the public URL is justified. Prefer changing only the title when it is not.
3. Move the file with Git so history remains understandable.
4. Update navigation and all inbound links in the same change.
5. Run a dry-run publication and review the proposed create/delete pair.
6. Publish, verify the new URL, and arrange a redirect outside the managed page set when old external links must remain valid.

## Remove a page

1. Confirm that its useful content has moved or is no longer true.
2. Remove all inbound links and its navigation item.
3. Delete the Markdown file in the same change.
4. Run validation and a publication dry run.
5. Confirm that only the intended managed page is deleted.

Never delete a page merely because it is short. First decide whether it should be expanded, merged into a stronger task page, or retained as a useful landing page.

## CI and publication

The Wiki Documentation workflow runs publisher tests and validation for pull requests that change wiki or publisher files. A push to `master` publishes when repository variable `WIKI_PUBLISH_ENABLED` is `true`.

The `wiki-production` GitHub environment provides:

| Setting | Purpose |
| --- | --- |
| `WIKI_URL` secret | HTTPS root URL for the Wiki.js instance |
| `WIKI_API_TOKEN` secret | GraphQL access for page read/write/delete and navigation management |
| `WIKI_PUBLISH_ENABLED` repository variable | Enables automatic production reconciliation after validation |

Manual workflow dispatch supports dry runs, reviewed adoption of matching unmanaged pages, and an explicit mass-delete override.

## Review checklist

Before merging documentation:

- [ ] Verify behavior against the current source, schemas, built CLI, or UI rather than memory.
- [ ] Define unfamiliar terminology and placeholders.
- [ ] State prerequisites, supported platforms, expected results, and failure checks.
- [ ] Confirm commands are safe, complete, and run in the stated shell or directory.
- [ ] Confirm internal links and navigation pass local validation.
- [ ] Inspect tables, tabsets, callouts, code blocks, anchors, and diagrams in rendered Wiki.js.
- [ ] Inspect the page at wide desktop and narrow/mobile widths.
- [ ] Verify the live URL and navigation item after publication.
- [ ] Check that related pages do not now contain contradictory or duplicated instructions.

As documentation coverage automation is added, its source inventories and generated-reference checks become mandatory parts of this checklist.
