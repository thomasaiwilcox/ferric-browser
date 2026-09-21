# ADR-0003: System Qt as the V1 distribution baseline

Status: accepted for the current Arch target
Date: 2026-09-18

## Decision

Ferric Browser uses the distribution's dynamically linked Qt 6/WebEngine stack as
the V1 package baseline. The Arch recipe declares the supported Qt family and
minimum version, installs the browser executable and desktop resources in the
standard filesystem layout, and keeps portals, PipeWire, dictionaries, and
external tools optional. The Omarchy integration remains a separate data and
helper package.

## Rationale and boundary

The target host provides the required Qt 6.11.2 development/runtime packages,
and the checked-in package recipe already models the system-Qt layout. A
private Qt bundle would add resource deployment, security patching, licensing,
size, and release/CI ownership without evidence that the target distribution
cannot provide the required behavior. This decision does not claim that an
installed package is release-qualified; a clean Arch build/install and native
Wayland run remain release gates.

## Consequences

- Build and runtime Qt families must remain compatible.
- Package checks validate metadata, desktop integration, dependencies, and
  optional-component boundaries.
- The browser must report missing optional capabilities rather than silently
  treating them as engine or compilation failures.
- Revisit this decision only with a documented Qt security/behavior gap and a
  maintained replacement plan.
