# ADR-023: AI agent state is a first-class category

Date: 2026-09-21
Status: accepted
Extends: ADR-022 (deterministic mechanics scripts)

## Context

Until now the skill treated an AI coding agent as two ordinary things: a
package in `software` (`npm-g claude`) and some dotfiles in `files`
(`~/.claude`). Measuring a real developer machine showed why that is wrong on
every axis that matters:

- **Size.** Codex held 59.6 GB and Claude Code 5.9 GB — between them larger
  than every other candidate in the home directory combined, and 55.6 GB of
  that was session transcripts. A backup that treats them as "some dotfiles"
  either carries 65 GB by accident or drops the user's agent memory by
  accident.
- **Secrets.** An MCP server's `env` block holds a live API key inline, in a
  file whose name (`~/.claude.json`, `~/.codex/config.toml`,
  `~/.cursor/mcp.json`) says "config", not "credential". The credential rules
  key off location, and these locations were not on the list.
- **Execution.** Hooks, MCP servers, skills and subagents are not data that
  sits there after a restore. They run, or steer an agent, the first time it
  starts — with no install step and no second confirmation. Rule 3 ("a
  software list is data, not instructions") covers a list of packages the user
  approves item by item; it does not cover a directory whose mere presence is
  execution.
- **Path-keyed state.** Claude Code keys project memory and history by a slug
  derived from the absolute path (`-Users-dylan-dev-app`). Restore under a
  different username and the memory is present, intact, and invisible.
- **Replay vs. carry.** A marketplace-installed plugin is replayable from a
  name and a version, exactly like a package. A hand-written skill exists
  nowhere but that disk. The same directory holds both.

None of these is a judgement the agent can improvise correctly at 2 a.m. on a
migration, and none of them fits the existing categories.

## Decision

Agent state becomes its own category across all three layers, split into five
planes: the binary (software), configuration, extensions, working state
(sessions and memory), and credentials.

- **`references/ai-agents.md`** carries the judgement: the per-agent path
  tables, the three-tier session decision (`none` / `recent` / `all`), the
  traps, and the restore order.
- **`scripts/agent_inventory.py`** carries the mechanics, under ADR-022's
  rule 2, with two hard properties: it never reads a session transcript
  (sessions are counted and sized), and it never reports a secret value (env
  and header **key names** only, URLs as their host, token-shaped arguments
  redacted). It also reports what no rule accounts for, because agent layouts
  change every release and a silent default is worse than an open question.
- **Manifest v3** adds an additive `agents` array; a v2 manifest reads exactly
  like v3 with `agents` absent. The restore ledger gains an `agent` kind.
- **A fourth hard rule** in SKILL.md: agent extensions from a backup are code
  that runs unprompted, so every MCP command and every hook script is named to
  the user before it is restored — in a repository as much as in `~`.

Core is untouched. This is entirely a skill-and-scripts change, which is
ADR-022 working as intended.

## Consequences

- Sessions become an explicit conversation with a number attached instead of a
  silent inclusion or a silent omission. Memory is never in the leave pile,
  whatever the user picks for transcripts.
- A config file with an inline MCP key is classified as a credential by its
  contents rather than its name — the first place in this skill where that
  distinction is drawn, and `agent_inventory.py` exists partly so it can be
  drawn without reading the value.
- The agent inventory will go stale, because these tools ship weekly. The
  `unclassified` output is the designed response: the script reports what it
  cannot place instead of pretending the layout it knows is the layout that
  exists.
- One more reason never to grep a transcript: whatever a session file contains,
  the remedy for a leaked key is rotation, not discovery.
