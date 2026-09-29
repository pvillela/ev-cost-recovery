# Claude Doctor

## Proposed actions

- Check 0 (setup health): no fixes needed.
  - One native install (2.1.284). No npm leftovers.
  - ~/.local/bin is in PATH. It appears twice, which does no harm.
  - All settings files parse. You have no agent definitions, and the skill frontmatter is valid.
- Check 1 (unused extensions):
  - Add "skillOverrides" to ~/.claude/settings.json, setting each of the 9 anthropic-skills:* names above to "off".
    - This changes the CLI only. claude.ai on the web is not affected.
    - I am not certain the override key for synced skills is the anthropic-skills: form, so I will check the skill list in a new session.
  - Run /mcp disable claude.ai Claude Docs. You must type this yourself because it is a slash command. It applies to this project only.
- Check 2 (local vs checked-in CLAUDE.md): no duplicates and no contradictions.
- Check 3 (derivable content): the project CLAUDE.md is already lean. It holds gotchas and rationale, not layouts or stack lists. It is 10k chars, well under the 50k-char warning threshold.
- Check 4 (lazy loading): nothing to move. The rename/feature checklist has to be loaded before a rename starts, so a skill that loads on request would come too late.
- Check 7 (version): 2.1.284 is the latest version on the latest channel. You turned off background updates (autoUpdates: false), so run claude update from time to time.
- Check 8 (default mode): add "permissions": {"defaultMode": "auto"} to ~/.claude/settings.json.
  - This applies to every project.
  - Your action-guard hook still runs in auto mode.
  - If auto mode is unavailable at startup, the CLI goes back to default mode and shows a notice.
- Check 9 (pre-approvals): none proposed.
  - The read-only denials were one-off grep -n/sed -n/cat strings with kind automode-unavailable, which means the auto-mode checker was down at the time. These commands are normally allowed without a prompt, so an exact rule would never match again.
  - The one git show denial came from your own rule, so I left it alone.

## Warnings

- Check 5 (hooks): all hooks are fast.
  - PreToolUse action guard: 17 ms typical and 75 ms maximum over 506 runs.
  - SessionStart startup: 63 ms typical and 209 ms maximum.
  - The rustfmt PostToolUse hook has no timing data. From its config it is a light formatter call.
- Check 6 (context use): the largest always-loaded items are the project CLAUDE.md (~2.5k), the synced skill listing (~1.6k), MEMORY.md (~830) and Claude Docs (~800). The skill listing is well within its ~10k budget. Run /context for exact live figures.
- Note: ~/.claude/settings.json already has uncommitted changes in your ~/.claude git repo. My edits would be added to them.

## After implementation

### Changes

- Skills off: a new skillOverrides block sets the 9 unused Anthropic skills (anthropic-skills:docs, docx, google-workspace, import-memory, morning, pdf, pptx, skill-creator, xlsx) to "off".
  - This changes the Claude Code command-line tool only. claude.ai on the web is not affected.
  - Not confirmed yet: I am not sure the anthropic-skills: name form is the right key for skills synced from your account. In a new session, check whether those 9 skills are still in the skill list. If they are, tell me and I'll change the key.
- Auto mode: permissions.defaultMode is now "auto". This applies to every project, and your action-guard hook still runs.
- Your step: type /mcp disable claude.ai Claude Docs to turn off the unused Docs connector. It applies to this project only.

### Other results

- Your version, 2.1.284, is the latest.
- The install, settings files and hooks are healthy.
- I found nothing to cut or move in the CLAUDE.md files.
- I proposed no command pre-approvals.

## How to undo

- Delete the skillOverrides block, or delete the "defaultMode": "auto", line.
- For the connector, type /mcp enable claude.ai Claude Docs.
- settings.json already had uncommitted changes in your ~/.claude git repo. git -C ~/.claude diff settings.json shows my changes together with yours.
