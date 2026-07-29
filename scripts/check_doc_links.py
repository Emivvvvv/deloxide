#!/usr/bin/env python3
"""Validate local Markdown links and heading anchors without network access."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit


MARKDOWN_LINK = re.compile(r"!?\[[^\]]*\]\(([^)\s]+)(?:\s+[^)]*)?\)")
HTML_LINK = re.compile(r"\b(?:href|src)\s*=\s*([\"'])(.*?)\1", re.IGNORECASE)
ATX_HEADING = re.compile(r"^ {0,3}#{1,6}[ \t]+(.+?)(?:[ \t]+#+)?[ \t]*$")


def github_anchor(heading: str) -> str:
    """Lowercase a heading, remove formatting/punctuation, and hyphenate spaces."""
    text = re.sub(r"`([^`]*)`", r"\1", heading)
    text = re.sub(r"!?\[([^]]*)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"<[^>]+>", "", text)
    characters = []
    for character in text.lower():
        if character.isalnum() or character == "-":
            characters.append(character)
        elif character.isspace():
            characters.append(" ")
    return re.sub(r" +", "-", "".join(characters).strip())


def collect_anchors(markdown: str) -> set[str]:
    """Return anchors for ATX headings, suffixing duplicate anchors with -1, -2."""
    anchors: set[str] = set()
    counts: dict[str, int] = {}
    in_fence = False
    for line in markdown.splitlines():
        if re.match(r"^\s*(```|~~~)", line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        match = ATX_HEADING.match(line)
        if not match:
            continue
        anchor = github_anchor(match.group(1))
        if not anchor:
            continue
        number = counts.get(anchor, 0)
        candidate = anchor if number == 0 else f"{anchor}-{number}"
        while candidate in anchors:
            number += 1
            candidate = f"{anchor}-{number}"
        counts[anchor] = number + 1
        anchors.add(candidate)
    return anchors


def markdown_files(inputs: list[Path]) -> list[Path]:
    """Expand file inputs directly and directory inputs recursively to *.md."""
    files: set[Path] = set()
    for input_path in inputs:
        if input_path.is_dir():
            files.update(path for path in input_path.rglob("*.md") if path.is_file())
        elif input_path.is_file() and input_path.suffix.lower() == ".md":
            files.add(input_path)
    return sorted(files)


def _links(line: str) -> list[str]:
    return [match.group(1) for match in MARKDOWN_LINK.finditer(line)] + [
        match.group(2) for match in HTML_LINK.finditer(line)
    ]


def _local_destination(destination: str) -> tuple[str, str] | None:
    parsed = urlsplit(destination)
    if parsed.scheme or parsed.netloc:
        return None
    path = unquote(parsed.path)
    fragment = unquote(parsed.fragment)
    if path.startswith("/") and not fragment:
        return None
    return path, fragment


def validate_paths(files: list[Path]) -> list[str]:
    """Return path:line diagnostics for missing relative targets or anchors."""
    diagnostics: list[str] = []
    anchor_cache: dict[Path, set[str]] = {}
    for source in markdown_files(files):
        try:
            lines = source.read_text(encoding="utf-8").splitlines()
        except OSError as error:
            diagnostics.append(f"{source}: unable to read file: {error}")
            continue
        for line_number, line in enumerate(lines, start=1):
            for destination in _links(line):
                local = _local_destination(destination)
                if local is None:
                    continue
                raw_path, fragment = local
                target = source if not raw_path else source.parent / raw_path
                if raw_path and not target.exists():
                    diagnostics.append(
                        f"{source}:{line_number}: missing local target {raw_path}"
                    )
                    continue
                if fragment and target.suffix.lower() == ".md":
                    resolved_target = target.resolve()
                    if resolved_target not in anchor_cache:
                        try:
                            anchor_cache[resolved_target] = collect_anchors(
                                target.read_text(encoding="utf-8")
                            )
                        except OSError as error:
                            diagnostics.append(
                                f"{source}:{line_number}: unable to read target {raw_path}: {error}"
                            )
                            continue
                    if fragment not in anchor_cache[resolved_target]:
                        description = raw_path or source.name
                        diagnostics.append(
                            f"{source}:{line_number}: missing anchor #{fragment} in {description}"
                        )
    return diagnostics


def main(argv: list[str]) -> int:
    """Print diagnostics and return 1 on failure, otherwise return 0."""
    diagnostics = validate_paths([Path(argument) for argument in argv])
    if diagnostics:
        print("\n".join(diagnostics), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
