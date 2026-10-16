"""Validate the checked-in editor definitions without launching an editor."""

import json
from pathlib import Path


root = Path(__file__).resolve().parent.parent
editor = root / "editors/vscode"
package = json.loads((editor / "package.json").read_text())
grammar = json.loads((editor / "syntaxes/oche.tmLanguage.json").read_text())
configuration = json.loads((editor / "language-configuration.json").read_text())

language = package["contributes"]["languages"][0]
registered_grammar = package["contributes"]["grammars"][0]
assert language["id"] == "oche"
assert ".oche" in language["extensions"]
assert (editor / language["configuration"]).is_file()
assert registered_grammar["language"] == "oche"
assert registered_grammar["scopeName"] == grammar["scopeName"] == "source.oche"
assert (editor / registered_grammar["path"]).is_file()
assert configuration["comments"]["lineComment"] == "//"

serialized_grammar = json.dumps(grammar)
for token in (
    "fn",
    "struct",
    "enum",
    "comptime",
    "const",
    "mut",
    "match",
    "try",
    "import",
    "dart",
    "splice",
    "quote",
    "Option",
    "Result",
):
    assert token in serialized_grammar, f"syntax grammar does not cover {token}"

print("VS Code syntax and language registration: passed")

zed = root / "editors/zed"
zed_manifest = (zed / "extension.toml").read_text()
zed_config = (zed / "languages/oche/config.toml").read_text()
zed_highlights = (zed / "languages/oche/highlights.scm").read_text()

assert 'id = "oche"' in zed_manifest
assert "[grammars.oche]" in zed_manifest
assert "[language_servers.oche]" in zed_manifest
assert 'name = "Oche"' in zed_config
assert 'path_suffixes = ["oche"]' in zed_config
assert 'line_comments = ["// "]' in zed_config
for token in (
    "fn",
    "struct",
    "enum",
    "comptime",
    "const",
    "mut",
    "match",
    "try",
    "import",
    "dart",
    "splice",
    "quote",
    "Option",
    "Result",
):
    assert token in zed_highlights, f"Zed highlights do not cover {token}"

print("Zed syntax and language-server registration: passed")
