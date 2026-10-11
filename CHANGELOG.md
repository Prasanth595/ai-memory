# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `install-mcp --client prime-agent` registers ai-memory with Prime Agent
  (PrimeIntellect-ai/prime-agent), merging an HTTP entry into the `mcpServers`
  map of `$PRIME_AGENT_CODING_AGENT_DIR/settings.json` (default
  `~/.prime/agent/settings.json`) and preserving other settings and servers.
  With a token the entry names `"bearerTokenEnvVar": "AI_MEMORY_AUTH_TOKEN"`
  and the token is not written; Prime Agent 0.10 counts a user HTTP server as
  connected, and lists it to the model, only with that variable set (or an
  OAuth grant), so export it where you launch `prime-agent`. Without a token
  the server is still callable through `mcp.call_tool("ai-memory", ...)` but is
  not listed. MCP-only: Prime Agent 0.10 has no lifecycle hooks. (#1163)
- `memory_write_page` accepts `expected_page_id` (write only if that is still
  the page's latest version) or `create_only` (write only if the page does not
  exist), and `memory_delete_page` accepts `expected_page_id`. A precondition
  that no longer holds fails with `reason: "precondition_failed"` and the
  current version, and changes nothing. `memory_read_page` returns the version
  as `page_id` and `/api/v1` pages as `id`. This is the compare-and-write the
  team-wiki sync needed (#986).
- The `ai-memory-wikisync` companion gained CI and post-merge integration
  (#986, slice 5): `sync --check` writes nothing and exits 0 in sync, 3 on
  drift and 4 on conflicts or refusals (a clone without sync state compares
  the repository with the server directly), and `install-hook` /
  `uninstall-hook` manage a marked git `post-merge` block that runs `sync`
  after each merge: a dry-run report unless `--on-merge apply`, never with
  `--prefer`, and never with a stored token. The cookbook has a GitHub
  Actions recipe. (#986)
- The `ai-memory-wikisync` companion now propagates deletes with
  `sync --propagate-deletes` (#986, slice 4): a file deleted in the repository
  deletes its server page through `memory_delete_page`, and a page deleted on
  the server deletes its file, when the other side is unchanged since the last
  sync. A delete against an edit is a conflict that `--prefer` resolves, more
  than `--max-deletes` (default 10) refuses the run, and a pinned page is
  deleted only with `--prefer repo`. Every sync write and delete now carries
  the page version it was planned against, so a page changed during the run is
  reported and left alone while the rest apply, and `sync --apply` refuses a
  server older than 2.7. Without the flag deletes are still only reported.
  (#986)
- `ai-memory doctor` now reports three silent capture failures: an unreachable
  server (reported instead of aborting the check), a hook spool that is full and
  evicting undelivered events, and installed ai-memory Codex hooks that Codex
  skips because they are untrusted or changed since they were trusted (asked
  of Codex itself through `codex app-server`). `ai-memory status` no longer says
  queued events will be delivered once the spool has started evicting them.
- Added two-way sync to the `ai-memory-wikisync` companion (#986, slice 3):
  `sync` imports repository edits and new pages through the public
  `memory_write_page` MCP tool and exports server edits, comparing each
  page with the last sync. A page changed on both sides is a conflict until
  `--prefer repo` or `--prefer server`; deletes are reported, not synced.
  It is a dry-run without `--apply`, refuses files without frontmatter,
  refuses to overwrite server pages whose metadata a write would clear, and
  re-reads each page right before writing it. (#1164)
- `finalize-session --agent antigravity-cli` now replays the session's typed
  prompts from `agy`'s `history.jsonl` before its synthetic session-end, so
  live Antigravity sessions, whose hooks carry no prompt event, get a summary
  page titled by the first prompt, a handoff with "Started/Last", and prompts
  that `memory_query` and `memory_recent` can find. Only lines whose
  conversation and workspace match the session are read; each prompt carries
  the key `backfill` mints for it, so a repeated `--reopen` stores it once and a
  prompt typed after the first finalize arrives with the next one. A checkout
  the capture allowlist does not admit and `--all-owners` runs replay nothing,
  and a missing `history.jsonl` only warns. (#1160)
- `ai-memory run` accepts `copilot` (alias `copilot-cli`) and manages GitHub
  Copilot CLI as a workstream harness: a fresh session gets a generated
  `--session-id`, a returning one is resumed with `--resume=<id>`, user
  selectors (`--resume`, `--continue`, `--session-id`, `--connect`) win,
  utility subcommands pass through, and `--yolo` maps to Copilot's native
  `--yolo`. Visible messages, tool calls and results, and compaction summaries
  are imported read-only from `$COPILOT_HOME/session-state/<uuid>/events.jsonl`
  (system prompts, hidden reasoning, hook output, and telemetry are excluded),
  and the existing SessionStart hook delivers the workstream context. The
  first managed launch auto-wires Copilot's hooks and MCP, and `doctor` now
  counts its local sessions. Verified against Copilot CLI 1.0.92. (#1040)
- GitHub Copilot CLI is now a first-class routing target:
  `install-skills --agent copilot-cli` (and `install-instructions
  --skills-agent copilot-cli`) writes the managed Agent Skills to the
  repository's `.github/skills` or, globally, to `$COPILOT_HOME/skills`
  (default `~/.copilot/skills`); `memory_install_self_routing` returns
  `copilot_cli` filename and skill-root hints; the routing snippet, the
  routing-install skill, and the MCP instructions name Copilot CLI with
  `AGENTS.md`; and `uninstall` sweeps both Copilot roots, removing only
  marker-bearing ai-memory skills. (#1040)
- Added `ai-memory recover` and a bounded local recovery journal for server
  outages. Offline managed launches now record only uniquely correlated native
  session locators; recovery quarantines overlapping spool entries before exact
  `/hook/batch` transcript replay, journals harnesses without transcript export
  as spool-only without calling their exporter, retains bounded recovery dedup
  keys with their sessions,
  fails closed on terminal drops, and lets only the original owner finish an
  expired unsuperseded managed run through exact idempotent replay. Backfill
  and recovery replay now use reserved `recovery_*` idempotency keys, so
  re-running `backfill --force` over sessions imported by ai-memory ≤2.6.0 (or
  plain-backfilling a recovery-replayed session) can duplicate those older
  observations. (#1125)
- Added `AI_MEMORY_LLM_COMPAT_DISABLE_THINKING` for the `openai-compat`
  provider (opt-in, off by default, ignored by every other provider): when
  set, every chat request carries
  `chat_template_kwargs: {"enable_thinking": false}`, so thinking-capable
  local engines (vLLM / SGLang serving Qwen3-class models) spend the output
  budget on the structured payload instead of a reasoning pass. (#1130)
- Added the `cursor` LLM provider. Consolidation calls the logged-in Cursor
  Agent CLI (`agent --print --mode ask`) so a Cursor subscription can drive
  summaries without an API key. The default model is `cursor-grok-4.6-high`
  ("Grok 4.6"). `AI_MEMORY_CURSOR_AGENT` overrides the binary. The provider never passes
  `--yolo` or `--force`, because the prompt is captured session text. (#1127)
- Added `applies_to` and `enforced_by` arguments to `memory_write_page`, and
  `--applies-to` / `--enforced-by` to `ai-memory write-page`, for profile
  entries. Both fields were documented as profile-entry frontmatter, but no
  tool could set them, so scoping an entry to a stack or keeping it out of the
  digest meant editing the wiki file by hand, and the next tool rewrite of the
  entry dropped the value. A tag the profile cannot detect in any project
  (`typscript`) is refused instead of silently hiding the entry, and both
  arguments are refused on any page the digest does not read: one outside
  `profile/`, or a `profile/` path in an ordinary project. (#1178)

- Added the server version to the footer of every signed-in `/web` page (the
  public login and change-password pages leave it out), on the right
  side opposite "ai-memory · read-only", so you can tell which build a browser
  tab is talking to without running `ai-memory status`. (#1171)

- Added `install-mcp --client dsh` for the DeepSeek Harness (aliases
  `deepseek-harness` and `deepseek_harness`). DSH keeps its MCP client in
  a Cordis profile patch rather than a fixed config file, so the command prints a
  ready-to-merge `- insert:` fragment for `@deepseek-ai/dsh-mcp-client` and
  refuses `--apply`. (#1193)

### Changed
- Rotated older release notes out of the root `CHANGELOG.md` into immutable
  per-version files under `docs/changelog/`; the active changelog keeps
  `## [Unreleased]` plus the two most recent releases. `bin/release` performs
  rotation automatically.
- Changed `ai-memory-wikisync` files to carry a small frontmatter (`title`,
  plus `tags`, `pinned` and a non-default `tier` when set) above the body,
  so metadata survives a round trip. The first run after upgrading rewrites
  unedited files of an earlier export in place. (#1164)

### Fixed
- Fixed SessionEnd consolidation repeating the same expensive prompt on the
  durable queue when the structured LLM response fails deterministically: a
  truncated or empty response, malformed JSON, or an unexpected response
  shape now ends the queue on the first attempt, keeping the heuristic page
  the hook already wrote; pre-send connection and capacity (`503`) failures
  retain the bounded backoff. (#1131)
- Fixed structured LLM responses stopped at the output budget
  (`finish_reason = "length"`) or returned without usable content: they now
  fail with redacted terminal errors, without copying the response. (#1130)
- Fixed the Jev choice reranker adapter silently fabricating scores from a
  malformed judge response: `docs/examples/jev-reranker-adapter/jev_rerank_shim_choice.py`
  defaulted missing or misnamed `probabilities` entries to `0.0`, so an HTTP
  200 with a broken payload produced a well-formed but meaningless ranking
  the server could not tell apart from a real judgement. The shim now
  validates the response (key set matches the candidate list, no duplicate
  JSON keys, numeric non-boolean values in `[0, 1]`, NaN/Inf rejected,
  distribution sum near 1) and answers HTTP 500 on any violation, which is
  the failure mode ai-memory already degrades safely on. Includes stdlib
  `unittest` coverage in `test_jev_rerank_shim_choice.py`. (#1198)
- Fixed Hermes Agent tool events being dropped under `capture_mode =
  allowlist`. Hermes reports its own process directory (usually `~`) as the
  payload `cwd` and points each tool at a repository with an absolute
  `workdir` or `path`, so the marker lookup never found the repository's
  `.ai-memory.toml`. The native `ai-memory hook` now also routes a shell
  command by its absolute `workdir` and a search/list tool by its absolute
  `path`, alongside the file-tool targets it already followed, when that
  location proves one other repository or marker; relative locations still
  keep the payload `cwd`. A reroute is now refused when the payload `cwd`'s own
  `[capture] ignore_paths` would drop the event, so an exclusion that names a
  path outside its checkout, or a source file read by a command whose
  `workdir` points at another project, is no longer judged only by the
  destination's policy. (#1199)

## [2.6.3] - 2026-10-09

### Fixed
- Fixed cross-project message tools acting on another project's mailbox. A
  static MCP client (one that does not forward its lifecycle-hook session id,
  such as Claude Code's MCP connection) that called `memory_message_pop`,
  `memory_message_cancel` or `memory_message_send` without `workspace` and
  `project` got whichever project published to the shared active-project slot
  last, often one open in another harness: the pop claimed that project's
  message, a cancel without an id cleared its whole outbox, and a send went out
  under its name as the sender the recipient judges trust by. The three tools
  now refuse a scope the server inferred, naming the project it would have used
  and how, and change nothing; an explicit scope or a caller bound to its own
  hook session proceeds. `memory_message_list` always reports `resolved_scope`,
  `scope_source` and a hint when it inferred the project, and the SessionStart
  inbox notice and `memory_briefing` (new `scope` field) name the inbox to pass.
  (#1197)
- Fixed a purged session coming back. `purge-session` leaves a tombstone so a
  late event cannot recreate the session, but live hook ingest creates its
  session row on a path that never checked it, so the next event for a purged
  session brought back the session and a new observation. Hook ingest now
  refuses it, and the delivery is acknowledged as `dropped_invalid` instead of
  failing and being retried from the client's spool. (#1194)
- Fixed `purge-session` leaving the user's own sentences behind in the
  cross-project profile's evidence: the profile harvests preference-shaped
  sentences from prompts into candidate rows keyed to the session, and the
  purge removed the session but not those rows, so a later pass could still
  converge a purged sentence into a profile page. The purge now deletes the
  session's prompt candidates in the same transaction. (#1195)
- Fixed `move-project` leaving rows on the old workspace. Pending
  cross-project messages (addressed to the project and sent by it), entities,
  page feedback, session purge tombstones and the profile evidence were not
  re-stamped with the project's other tables, so the moved project lost its
  pending mail, its entity retrieval and open feedback findings, and a purged
  session could be recreated. They now move in the same transaction. (#1196)
- Fixed a Codex, Pi, OMP or Grok session going missing once its store held more
  than 2,000 transcripts. Looking a session up by id read the first 2,000 files
  in directory-walk order, so the rest came back as not found: `ai-memory run`
  dropped the linked session and started a fresh one, and a transcript import
  failed. The lookup now reads the transcript that names the session id first,
  then the newest. (#1189)
- Fixed Crush sub-agent sessions being listed as sessions of their own:
  `backfill`, `doctor` and the automatic session pick of `ai-memory run` read
  the Crush `sessions` table without skipping rows that have a parent, so a
  sub-agent session could be imported as a separate session or chosen for the
  resume. The listing now skips them, as session discovery already did.
  (#1190)
- Fixed rules titled in a script without ASCII letters (Cyrillic, CJK, Arabic
  and others) overwriting each other: multi-page consolidation named every such
  rule page `_rules/rule.md`, so only the last survived in the live rules list.
  Each rule now gets a `_rules/rule-<hash>.md` name derived from its title. Rule
  titles in Latin scripts are named as before. A restated non-Latin title
  (case, spacing or punctuation aside) updates its own page. An existing
  `_rules/rule.md` is left in place; delete it once its rule is restated.
  (#1191)
- Fixed auto-improve accepting a proposal whose confidence is outside 0 to 1:
  a model that answered in percent (`85`) cleared the confidence floor and was
  staged, shown to the reviewer as 8500% and ranked first when sorting by
  confidence. Such a value is now rejected as `confidence_out_of_range` and
  recorded with the other rejected candidates. (#1192)
- Fixed the curl hook installer (`ai-memory-install-hooks`) installing event
  scripts that could not run. Every script sources the shared `_lib.sh` helper
  from its own directory or the one above it, but the installer fetched only
  the per-agent scripts, so the helper was never written and each script
  stopped at the line that sources it. The installer now also extracts
  `_lib.sh` from the verified archive into the install root, beside the agent
  directories, and refuses to continue when the archive does not contain it.
  (#1184)
- Fixed the curl hook installer (`ai-memory-install-hooks`) disagreeing with
  the hook bundles about which scripts an agent has. `--agent antigravity-cli`
  always failed: it asked for the seven default event scripts, the Antigravity
  bundle ships four, and the install stopped at the first missing one.
  `--agent claude-code` and `--agent grok` never installed `subagent-start.sh`
  and `subagent-stop.sh`, which their hook config points at, so `install-hooks`
  warned that both were missing. The installer now installs the scripts each
  bundle ships, and a test checks every script-based agent against the
  repository bundles. (#1185)
- Fixed Kiro CLI tool calls being stored with no title or content: Kiro's
  `PreToolUse` and `PostToolUse` hooks carry `tool_name` and `tool_input`
  (plus `tool_response` after the call), and the capture policy already read
  them, but Kiro was missing from the tool-capture mapping, so every Kiro tool
  observation reached the store with a generic title and an empty body. They
  now get the same tool-family title and output summary as Claude Code's.
  (#1187)
- Fixed the generated OpenCode, OpenCode 2, OMP, Pi and OpenClaw integrations
  losing hook events when the server is saturated. They treated every 4xx as a
  permanent rejection, so a `429` (or a `408` or `425`) was dropped instead of
  spooled, and their spool drain deleted each queued entry it met on such a
  response. Both now keep `408`, `425`, `429` and every `5xx` for a later
  drain, as the shell, PowerShell and native hooks already do. Re-run
  `install-hooks --apply` for the agent to regenerate its plugin. (#1188)
- Fixed Claude Code's Windows `PowerShell` tool being classified as an
  unknown tool, so its output never reached the session record and
  `[capture] ignore_paths` never checked its command. It is now a shell tool
  like `Bash`, in the native hook, the server, and the generated TypeScript
  integrations, and the shell argument reader also follows PowerShell's
  `-Param:value` binding (`Get-Content -Path:docs\adr\x.md`). (#1186)
- Fixed Zero's `sessionEnd` never delivering spooled events even with
  network allowed in Zero's sandbox. Zero kills a hook's detached children
  when the hook exits, which took the background drainer down with it. For
  Zero, `session-end` now drains in-process within a fixed 10-second budget
  (hard stop at 20 seconds, inside Zero's 30-second hook timeout); events
  that cannot be delivered stay spooled. Other agents keep the detached
  drainer. (#1172)
- Fixed generated TypeScript integrations (OpenCode, OpenCode 2, OMP,
  Pi, OpenClaw) flashing a console window on Windows.
  `discoverRemoteIdentity` spawned `git config --get remote.<name>.url`
  without `windowsHide`. The spawn now passes `windowsHide: true`, as
  the repo-root probe's two git spawns already do (#1169).
- Fixed `AI_MEMORY_LLM_REASONING_EFFORT` being ignored for
  `claude-haiku-5-5`: the Anthropic provider dropped effort and thinking
  fields for every Haiku model because Haiku 4.5 rejects them, so Haiku 5.5
  always ran adaptive thinking at its default `medium` effort. Haiku 5.5 and
  later now receive `output_config.effort`, adaptive thinking, and
  `thinking: disabled` for `none`, like the other current Claude models
  (#1180).
- Documented that Anthropic currently refuses Claude Free/Pro/Max subscription
  tokens from third-party apps, so `anthropic-oauth` usually fails with a
  `429 rate_limit_error` on every request. That is a policy refusal, not a
  real rate limit; the docs now point to the `anthropic` provider with a
  Console API key or another provider. Also corrected the claim that
  `claude setup-token` writes `CLAUDE_CODE_OAUTH_TOKEN`: it only prints the
  token, which you export yourself. (#1170)
- Documented that Zero runs hook commands inside its sandbox, whose default
  policy blocks network (loopback included) and writes outside the
  workspace, so hook events stay in the spool instead of reaching the
  server. The `install-hooks --agent zero` and `setup-agent` notes and the
  Zero install docs now name the global `~/.config/zero/config.json`
  `sandbox` keys that allow delivery, their tradeoff, and the
  `ai-memory hook-drain` fallback. (#1172)
- Fixed a global `~/.codex/AGENTS.md` reaching the cross-project profile as
  the user's own words. Codex injects its instruction files as a user message
  headed `# AGENTS.md instructions for <cwd>` when a project `AGENTS.md`
  contributes, and `# AGENTS.md instructions` with no directory when only the
  global file does. The transcript import recognized only the first heading,
  so `backfill` stored the global file as a user prompt in every project, and
  the profile harvest admitted its "always"/"never" lines, which repeat across
  projects by construction. The import now recognizes both headings. (#1173)
- Fixed the profile harvest reading text the user did not write as the
  user's words: a user-prompt observation that is a harness turn, such as
  Claude Code's `<task-notification>` blocks or Codex's injected
  instructions, is now skipped, and ai-memory's own routing and
  recalled-history blocks inside a prompt are left out, including a block the
  16 KiB prompt cap cut before its end marker. `profile rebuild` also skips
  Codex instructions an earlier version stored as prompts, but it keeps the
  candidates already harvested from them, so those lines can still show as
  waiting in `profile review`. An entry an earlier version already admitted,
  or one those candidates reach later, stays until `ai-memory profile forget`
  removes it. (#1173)
- Fixed Cursor tool calls being stored with no title or content: Cursor's own
  hooks, and the Claude Code hooks Cursor also runs, send Claude's
  `tool_name`/`tool_input` fields, but Cursor was missing from the tool-capture
  mapping, so every Cursor tool observation reached the store empty. They now
  get the same tool-family title and output summary as Claude Code's.
- Fixed every Cursor window adding a session to memory before any
  conversation: Cursor fires `sessionStart` for its empty draft composer with
  the placeholder id `empty-state-draft` and no workspace, and each launch
  appended another observation to that one empty session. The server now
  acknowledges and drops that placeholder.
- Fixed `profile review` listing page metadata such as `Status:** Accepted` as
  habits waiting for promotion: a curated page's statement was its first line
  of prose, which on an ADR-style page is a `**Status:** Accepted` or
  `**Date:**` field. Statements now skip fields with an ADR metadata label
  (Status, Date, Deciders, Supersedes and the like) whatever their value; a
  labelled rule such as `**Package manager:** pnpm` is still a statement.
  Fields harvested before the fix no longer reach `profile review`; an entry already admitted to the
  profile stays until `ai-memory profile forget`. (#1175)
- Fixed `install-mcp --client claude-desktop` refusing to run on Linux:
  Anthropic ships Claude Desktop for Linux as a beta, and the command now
  writes `$XDG_CONFIG_HOME/Claude/claude_desktop_config.json` (default
  `~/.config/Claude/`) instead of requiring `--config-file`.
- Corrected the Claude Desktop documentation: Linux is an Anthropic beta for
  Debian-based distributions with its config at
  `~/.config/Claude/claude_desktop_config.json` (pass it to
  `install-mcp --client claude-desktop` with `--config-file`), the Code tab's
  local sessions are already captured by the Claude Code hooks, and Cowork
  capture is not claimed. The desktop-app research notes record what was
  verified live and what is still open. (#878)
- Fixed `ai-memory-wikisync` committing its per-clone export state: the
  `git add` of the destination that `export --apply` prints also staged
  `.ai-memory-wikisync/state.json`, so two clones would conflict on it and
  trust each other's baselines. The state directory now ignores itself with
  a `.gitignore` of `*`. A state committed by an earlier export needs one
  `git rm -r --cached <dest>/.ai-memory-wikisync`. (#1162)
- Fixed `bootstrap` reading a rules file twice when it is reachable under two
  names: on a case-insensitive filesystem `claude.md` opened `CLAUDE.md`
  again, and on any system a `CLAUDE.md` linked to `AGENTS.md` did the same.
  The doubled text inflated the token estimate, could push every source over
  the budget (`no input sources selected`), and repeated the rules in the
  prompt. Only names the directory lists are read now, and identical text is
  read once. (#1168)
- Fixed `backfill` and managed runs not finding a Claude Code transcript when
  the session's directory contains anything but letters, digits and `/`:
  Claude Code names a project's folder by turning every other character into
  `-`, but ai-memory replaced only `/`, so every Windows path and any path
  with `.`, `_` or a space missed it, and the bounded fallback scan could run
  out on a large store first. The folder name now follows Claude Code's rule,
  every project folder is probed for the exact transcript before the scan,
  and a subagent's `subagents/*.jsonl` sidechain, which carries its parent's
  session id, is never taken for the session's transcript. (#1167)
- Fixed the macOS menu bar companion's **Settings…** item opening the
  Settings window behind the frontmost app, so clicking it appeared to do
  nothing. The item now activates the app before opening the window, as
  **Show Status…** already did. (#1161)
- Fixed `ai-memory run` creating an empty twin project when a repository's
  folder name differs from its git remote (a `new-space-game` checkout of
  `github.com/acme/unknown-system`) and its project predates recorded
  identities: the run asked for the remote-derived name alone, so it missed
  the folder-named project, and once hook capture claimed that project every
  later run failed with `project '…' is ambiguous`. The launcher now sends the
  checkout's remote identity with the derived name and the server routes the
  run by identity, as capture does — which also lets runs in an
  already-affected repository open again. A user whose run cwd sat inside a
  project they could not write got a silently created twin project; they now
  get a 403. (#1182)

## [2.6.2] - 2026-10-08

### Fixed
- Fixed automatic handoffs carrying harness scaffolding as the user's
  request: a user turn that opens with a markup block whose opening tag has
  no attributes (for example the `<task-notification>` block Claude Code
  delivers when a background task, agent or workflow completes) no longer
  becomes the handoff's `Started:` / `Last:` summary, its `Continue from:`
  line, or its `Unresolved question:` line. The handoff now uses the last
  prompt the user actually wrote, and falls back to the no-prompt summary
  when every prompt in the session is such a block. Prompts that only
  contain markup later in the text, pastes with attributes such as
  `<pasted_content id="…">`, and stored observations are unchanged. (#1158)
- Corrected the `profile rebuild` guidance in `docs/cross-project-profile.md`
  and the command's help: it never removes an entry, and a candidate recorded
  by an earlier version keeps the classification it was recorded with, so
  entries 2.6.0 admitted under the looser rules survive the upgrade and a
  rebuild. The doc now gives the upgrade path, `profile review` then
  `profile forget` on each entry to drop. (#1155)
- Fixed a `workspace/project` label passed as the project failing to resolve
  (for example `workspace: "default", project: "myorg/myproject"`): when no
  project has that exact name, the label's own workspace and project are used,
  under the same access checks as passing them separately. A write never
  creates a project whose name contains `/` any more; it is refused with the
  hint instead. (#1152, #1154)
- Fixed every Pi turn failing with a 400 on Moonshot/Kimi models: the
  generated Pi extension registered the MCP tools with schemas containing
  `$ref`, which Moonshot rejects. Its bridge now requests the `?flavor=moonshot`
  schemas, with every reference inlined, which any provider accepts. Re-run
  `ai-memory install-hooks --agent pi --apply` to regenerate the extension.
  (#1157)

Older released versions are archived under [docs/changelog/](docs/changelog/README.md).
[Unreleased]: https://github.com/akitaonrails/ai-memory/compare/v2.6.3...HEAD
[2.6.3]: https://github.com/akitaonrails/ai-memory/compare/v2.6.2...v2.6.3
[2.6.2]: https://github.com/akitaonrails/ai-memory/compare/v2.6.1...v2.6.2
