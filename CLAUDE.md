@AGENTS.md

Read `AGENTS.md` before doing anything here. The line above imports it for Claude Code; a tool that
reads this file as plain text must open `AGENTS.md` itself.

## Claude Code-specific additions

- **Skills:** `ww3-port` holds the traps of translating WW3 to Rust; `/port-next` starts a routine.
- **Attribution:** the harness's `Co-Authored-By` stays in the commit trailer, never in code,
  comments, PR titles or documents.
- **On compaction, preserve:** which files changed, which gates ran and their result, and the
  `(v)`/`⚠` status of any number written.
- **`WW3/` is a submodule you read,** not a repository you work in.
