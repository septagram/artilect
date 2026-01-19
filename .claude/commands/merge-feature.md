---
description: Create a proper feature merge with issue ID and bookmark update
allowed-tools: Bash(jj status:*), Bash(jj log:*), Bash(jj diff:*), Bash(jj new:*), Bash(jj commit:*), Bash(jj bookmark:*)
argument-hint: [bookmark-name]
---

# Merge Feature Branch

Create a proper non-fast-forward merge commit for a feature branch with issue ID.

## Context

Feature bookmark to merge: $ARGUMENTS (or infer from current branch if not provided)

Current jj status:
!`jj status`

Recent commits:
!`jj log --limit 30`

Bookmarks:
!`jj bookmark list`

## Task

1. **Find the last commit with issue ID** (format `[A##]` in square brackets) on the feature branch
2. **Review all commits** since that last issue-tagged commit using `jj log` and `jj diff --stat`
3. **Identify any cleanup needed** (dead code, commented code, temporary files)
4. **Report findings** to user:
   - Summary of changes
   - Any items that should be cleaned up before merge
   - Suggested commit message with issue ID
5. **After user approval**, execute:
   - Any cleanup edits requested
   - `jj commit` for cleanup changes (if any)
   - `jj new <bookmark> @- -m "[A##] <message>"` to create merge
   - `jj bookmark set <bookmark> -r @` to update bookmark
   - `jj new` to create a fresh working copy (so Git tools show the merge correctly)
6. **Verify** the merge graph looks correct with `jj log`

## Issue ID Format

- Project slug is "A"
- Format: `[A##]` where ## is the issue number
- Example: `[A14]` for issue 14

## Notes

- This is for completing a significant unit of work, not for WIP commits
- For quick WIP commits, just use `jj commit -m "description"` directly
- The merge should be non-fast-forward to preserve branch history
- Always run `jj new` after the merge to create a fresh working copy (required for Git tools like Sublime Merge to properly display the merge commit)
