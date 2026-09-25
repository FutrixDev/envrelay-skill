#!/usr/bin/env python3
"""Find git repositories and report their state as facts.

git_classify.py PATH [PATH...]
git_classify.py --scan ROOT [--max-depth N] [--exclude NAME]...

Each repository is classified into one of the five states from
references/git-repos.md:

  complete-repository   HEAD resolves and `git status` works
  partial-repository    git recognises it but HEAD has no commit yet
  invalid-head          a .git is there but git errors on the repository
  ordinary-directory    no .git at all
  unknown               git is missing, or a command timed out

For functional repositories it also reports remotes, branch, head, and the
three counts that decide the backup strategy: uncommitted, unpushed, stashes.

The scan prunes inside every repository it finds (the outer repo carries the
inner ones) and inside cache directories. Every git command here is
read-only; choosing a strategy from these facts is the skill's job, not this
script's.
"""

import argparse
import json
import os
import subprocess
import sys

DEFAULT_PRUNE = {
    "node_modules", ".venv", "venv", "target", "build", "dist", ".next",
    ".cache", ".npm", ".cargo", ".rustup", ".gradle", ".m2", ".tox",
    "__pycache__", "Library", ".Trash",
}


class GitError(Exception):
    pass


class GitUnavailable(Exception):
    pass


def git(repo, *argv, timeout):
    try:
        done = subprocess.run(
            ["git", "-C", repo, *argv],
            capture_output=True,
            text=True,
            timeout=timeout,
            stdin=subprocess.DEVNULL,
            env={**os.environ, "LC_ALL": "C", "GIT_OPTIONAL_LOCKS": "0"},
        )
    except FileNotFoundError:
        raise GitUnavailable("git is not installed")
    except subprocess.TimeoutExpired:
        raise GitUnavailable(f"git {' '.join(argv)} timed out")
    if done.returncode != 0:
        raise GitError(done.stderr.strip().splitlines()[0] if done.stderr.strip() else f"git {argv[0]} failed")
    return done.stdout


def classify(path, timeout):
    report = {"path": path}
    if not os.path.lexists(os.path.join(path, ".git")):
        report["state"] = "ordinary-directory"
        return report

    try:
        git(path, "rev-parse", "--git-dir", timeout=timeout)
        status = git(path, "status", "--porcelain", timeout=timeout)
    except GitUnavailable as error:
        report["state"] = "unknown"
        report["detail"] = str(error)
        return report
    except GitError as error:
        report["state"] = "invalid-head"
        report["detail"] = str(error)
        return report

    report["uncommitted"] = sum(1 for line in status.splitlines() if line.strip())

    try:
        remotes = {}
        for line in git(path, "remote", "-v", timeout=timeout).splitlines():
            parts = line.split()
            if len(parts) >= 3 and parts[2] == "(fetch)":
                remotes[parts[0]] = parts[1]
        report["remotes"] = remotes
        report["stashes"] = sum(
            1 for line in git(path, "stash", "list", timeout=timeout).splitlines() if line.strip()
        )
    except GitUnavailable as error:
        report["state"] = "unknown"
        report["detail"] = str(error)
        return report
    except GitError as error:
        report["state"] = "invalid-head"
        report["detail"] = str(error)
        return report

    try:
        report["head"] = git(path, "rev-parse", "--verify", "HEAD", timeout=timeout).strip()
    except GitUnavailable as error:
        report["state"] = "unknown"
        report["detail"] = str(error)
        return report
    except GitError:
        # No commit under HEAD. An unborn branch is a functional-but-empty
        # repository; anything else is a repository git cannot make sense of.
        try:
            ref = git(path, "symbolic-ref", "HEAD", timeout=timeout).strip()
            report["state"] = "partial-repository"
            report["branch"] = ref.rpartition("/")[2]
            report["detail"] = "no commits yet"
            report["unpushed"] = 0
            return report
        except (GitError, GitUnavailable):
            report["state"] = "invalid-head"
            report["detail"] = "HEAD does not resolve to a commit"
            return report

    try:
        branch = git(path, "rev-parse", "--abbrev-ref", "HEAD", timeout=timeout).strip()
        report["branch"] = branch
        if branch == "HEAD":
            report["detached"] = True
        report["unpushed"] = int(
            git(
                path, "rev-list", "--count", "--branches", "--not", "--remotes",
                timeout=timeout,
            ).strip()
            or "0"
        )
    except GitUnavailable as error:
        report["state"] = "unknown"
        report["detail"] = str(error)
        return report
    except GitError as error:
        report["state"] = "invalid-head"
        report["detail"] = str(error)
        return report

    report["state"] = "complete-repository"
    return report


def scan(root, max_depth, prune):
    """Yields directories containing a .git entry, pruning inside each hit."""
    root = os.path.abspath(root)
    found = []
    seen = set()
    for dirpath, dirnames, _filenames in os.walk(root, topdown=True, followlinks=False):
        depth = 0 if dirpath == root else os.path.relpath(dirpath, root).count(os.sep) + 1
        if ".git" in dirnames or os.path.lexists(os.path.join(dirpath, ".git")):
            real = os.path.realpath(dirpath)
            if real not in seen:
                seen.add(real)
                found.append(dirpath)
            dirnames[:] = []
            continue
        if depth >= max_depth:
            dirnames[:] = []
            continue
        # Hidden directories are scanned two levels deep — enough to find
        # ~/.password-store and ~/.config/nvim — and pruned below that, where
        # they are overwhelmingly caches rather than repositories.
        keep_hidden = depth + 1 <= 2
        dirnames[:] = sorted(
            name
            for name in dirnames
            if name not in prune and (keep_hidden or not name.startswith("."))
        )
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("paths", nargs="*", help="repository paths to classify")
    parser.add_argument("--scan", metavar="ROOT", help="walk ROOT looking for repositories")
    parser.add_argument("--max-depth", type=int, default=6, help="scan depth cap (default 6)")
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        metavar="NAME",
        help="extra directory name to prune while scanning, repeatable",
    )
    parser.add_argument("--timeout", type=int, default=10, help="seconds per git command (default 10)")
    args = parser.parse_args()

    if bool(args.paths) == bool(args.scan):
        parser.error("pass repository paths or --scan ROOT, not both and not neither")

    if args.scan:
        paths = scan(args.scan, args.max_depth, DEFAULT_PRUNE | set(args.exclude))
    else:
        paths = [os.path.abspath(p) for p in args.paths]

    result = {"repos": [classify(path, args.timeout) for path in paths]}
    if args.scan:
        result["scanned_root"] = os.path.abspath(args.scan)
        result["max_depth"] = args.max_depth
    json.dump(result, sys.stdout, indent=2, sort_keys=True)
    print()


if __name__ == "__main__":
    main()
