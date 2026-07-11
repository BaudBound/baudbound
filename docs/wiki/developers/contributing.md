---
title: Contributing
description: Standards and workflow for contributing production-quality changes to BaudBound.
tags: [developers, contributing]
---
# Contributing

Start from a current `master` branch and keep changes scoped to one coherent behavior. Read the owning modules and existing tests before choosing an abstraction. Preserve unrelated work in a dirty tree.

Code must be modular, explicit, and production-ready. Do not add placeholders, silently ignored configuration, shell-based native-action shortcuts, duplicated sources of truth, or compatibility claims without implementations. Treat package parsing, secrets, approvals, filesystem changes, network listeners, and process control as security-sensitive code.

Add focused tests with the implementation and broader integration coverage when changing shared contracts. Run formatter, linter, type checking, unit tests, schemas, builds, and relevant browser or platform tests before review.

Update `docs/wiki` whenever user behavior, operations, contracts, configuration, or contribution workflow changes. Root README remains a concise project entry point; detailed documentation belongs in the wiki.
