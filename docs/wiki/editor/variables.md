---
title: Variables and Data
description: Use mutable values, node outputs, built-in variables, secrets, and derived metadata in BaudBound workflows.
tags: [editor, variables]
---
# Variables and Data

Reference values with `{{variable_name}}`. A field may contain a token alone to preserve its original type, or include tokens in text to interpolate string values.

## Variable sources

**User variables** are created and changed by Variable Operation nodes.

**Node outputs** are read-only values emitted by actions and triggers. Their names use the node's configured output prefix.

**Secrets** are read-only declarations resolved by the runner. The editor stores only the name, type, and description.

**Manifest variables** are read-only package values: `manifest_name`, `manifest_version`, `manifest_author`, `manifest_description`, `manifest_website`, `manifest_repository`, and `manifest_minimum_runner_version`.

**System variables** are runner values: `system_os`, `system_arch`, `system_hostname`, `system_user`, `system_locale`, `system_timezone`, `system_date`, and `system_time`.

## Derived metadata

Non-secret variables expose derived values such as `{{foo.$length}}` and `{{foo.$count}}` where the source type supports them. The `$` namespace distinguishes derived metadata from ordinary object properties by contract. A real object property with the same reserved name is not addressable through the derived-token form.

## Visibility controls

The Variables tab can move recently updated values to the top and hide derived, built-in, or system entries. These options only change the panel; they do not change exported data or runtime behavior.

## Types and conditions

Variables may be strings, numbers, booleans, lists, or objects. Condition editors used by If/Else and While support multiple comparisons and an invert switch. Keep numeric and boolean values typed rather than formatting them as strings before comparison.

## Secret safety

Derived metadata is not generated for secrets. Secret values selected for simulation remain in memory for that simulation session and are not saved into the project or package. See [Secrets](../runner/secrets.md).
