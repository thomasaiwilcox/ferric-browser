"""Shared classifier for Qt SDK include paths in build output and live argv."""
from __future__ import annotations

import os
import shlex
from pathlib import Path
from collections.abc import Sequence


INCLUDE_FLAGS = ("-I", "-isystem", "-iquote", "-idirafter", "-F")


def qt_include_escape(value: str | Sequence[str], prefix: Path) -> str | None:
    """Return the first Qt SDK include path outside ``prefix``, if any.

    Generated CXX-Qt include directories can live outside the SDK and remain
    valid. A path is treated as a Qt include only when one component begins
    with ``qt`` or ``Qt``. Symlink paths that cross the pinned SDK boundary
    are rejected regardless of their lexical name.
    """
    prefix = prefix.resolve()
    if isinstance(value, str):
        try:
            tokens = shlex.split(value, posix=True)
        except ValueError:
            tokens = value.split()
    else:
        tokens = list(value)

    for index, token in enumerate(tokens):
        path = None
        if token in INCLUDE_FLAGS and index + 1 < len(tokens):
            path = tokens[index + 1]
        else:
            for flag in INCLUDE_FLAGS:
                if token.startswith(flag) and len(token) > len(flag):
                    path = token[len(flag):].lstrip("=")
                    break
        if not path:
            continue
        candidate = Path(path)
        if not candidate.is_absolute():
            continue
        lexical = Path(os.path.abspath(candidate))
        resolved = candidate.resolve()
        lexical_inside = lexical == prefix or prefix in lexical.parents
        resolved_inside = resolved == prefix or prefix in resolved.parents
        if lexical_inside != resolved_inside:
            return str(candidate)
        if not lexical_inside and any(
            part.startswith("Qt") or part.lower().startswith("qt")
            for part in candidate.parts
        ):
            return str(candidate)
    return None
