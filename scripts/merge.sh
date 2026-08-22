#!/usr/bin/env bash
# Fast-forward merge a feature branch into main, then clean up the worktree.
#
# Usage:
#   scripts/merge.sh <branch-name>          # merge and clean up
#   scripts/merge.sh <branch-name> --dry    # show what would happen
#
# The branch must already be committed. This script:
#   1. Tries git merge --ff-only in main
#   2. If FF fails, squash-rebases the branch onto main
#   3. Removes the worktree and deletes the branch

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MAIN_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WORK_BASE="$HOME/work"

die() { echo "error: $*" >&2; exit 1; }

usage() {
    cat <<EOF
Usage: scripts/merge.sh <branch-name> [--dry]

Fast-forward merge a feature branch into main and clean up the worktree.

Steps:
  1. Tries git merge --ff-only in main
  2. If FF fails, squash-rebases onto main
  3. Removes the worktree and deletes the branch

Examples:
  scripts/merge.sh feature/phase2-nav-restructure
  scripts/merge.sh fix/entry-edit-and-navbar --dry
EOF
    exit 1
}

[ $# -lt 1 ] && usage

BRANCH="$1"
DRY_RUN=false
[ "${2:-}" = "--dry" ] && DRY_RUN=true

# Strip refs/heads/ prefix if provided
BRANCH="${BRANCH#refs/heads/}"

# Find the worktree for this branch
find_worktree() {
    local branch="$1"
    for dir in "$WORK_BASE"/khanatime-*/; do
        [ -d "$dir" ] || continue
        [ -e "$dir/.git" ] || continue
        local wt_branch
        wt_branch=$(git -C "$dir" branch --show-current 2>/dev/null || true)
        if [ "$wt_branch" = "$branch" ]; then
            echo "$dir"
            return
        fi
    done
    return 1
}

WT_DIR=$(find_worktree "$BRANCH") || die "No worktree found for branch '$BRANCH'"

echo "═══════════════════════════════════════════════════════════════"
printf '\033[1;33m  Merging %s into main\033[0m\n' "$BRANCH"
echo "  Worktree: $WT_DIR"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# Step 1: Try FF merge
echo "── Step 1: Attempting fast-forward merge..."
if $DRY_RUN; then
    echo "  [dry-run] Would run: git -C $MAIN_DIR merge --ff-only $BRANCH"
else
    if git -C "$MAIN_DIR" merge --ff-only "$BRANCH" 2>/dev/null; then
        echo "  ✓ Fast-forward merge succeeded!"
        echo ""
        echo "── Step 3: Cleaning up..."
        git -C "$MAIN_DIR" worktree remove "$WT_DIR" 2>/dev/null || true
        git -C "$MAIN_DIR" branch -d "$BRANCH" 2>/dev/null || true
        echo "  ✓ Worktree removed, branch deleted"
        echo ""
        echo "═══════════════════════════════════════════════════════════════"
        printf '\033[1;32m  Done! Please git pull in the main repo.\033[0m\n'
        echo "═══════════════════════════════════════════════════════════════"
        exit 0
    fi
    echo "  ✗ FF failed — branch has diverged from main"
fi

# Step 2: Squash rebase
echo ""
echo "── Step 2: Squash-rebasing onto main..."
if $DRY_RUN; then
    echo "  [dry-run] Would squash in worktree: $WT_DIR"
    echo "  [dry-run] Commands:"
    echo "    cd $WT_DIR"
    echo "    ORIG_SHA=\$(git rev-parse HEAD)"
    echo "    git reset --soft main"
    echo "    git commit -m \"<original message>\""
    echo "    git diff \$ORIG_SHA  # verify identical"
else
    cd "$WT_DIR"

    # Save the original commit message and SHA for diff verification
    ORIG_MSG=$(git log -1 --format="%s" HEAD)
    ORIG_SHA=$(git rev-parse HEAD)

    # Soft-reset to main, recommit as single commit
    git reset --soft main
    git commit -m "$ORIG_MSG"

    # Verify the tree content is identical (use SHA, no branch needed)
    DIFF=$(git diff "$ORIG_SHA" 2>/dev/null || true)
    if [ -n "$DIFF" ]; then
        echo "  ✗ WARNING: tree content differs after squash!"
        echo "  The diff is:"
        echo "$DIFF"
        echo ""
        # Save the original branch for recovery
        PREBASE_BRANCH="${BRANCH}_prebase"
        echo "  Saving original branch as $PREBASE_BRANCH..."
        git branch "$PREBASE_BRANCH" "$ORIG_SHA" 2>/dev/null || true
        echo ""
        echo "  Aborting — fix manually or retest."
        git reset --hard HEAD@{1} 2>/dev/null || true
        exit 1
    fi

    echo "  ✓ Squash complete, tree content verified"

    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    printf '\033[1;33m  Branch squashed onto main. Re-run the cycle:\033[0m\n'
    echo "  1. cd $WT_DIR"
    echo "  2. ./scripts/check.sh"
    echo "  3. touch test-me-please"
    echo "  4. Tell user to test"
    echo "  5. After approval: scripts/merge.sh $BRANCH"
    echo "═══════════════════════════════════════════════════════════════"
    exit 0
fi

echo ""
echo "═══════════════════════════════════════════════════════════════"
printf '\033[1;32m  Done! Please git pull in the main repo.\033[0m\n'
echo "═══════════════════════════════════════════════════════════════"
printf '\033[1;32m  Done! Please git pull in the main repo.\033[0m\n'
echo "═══════════════════════════════════════════════════════════════"
