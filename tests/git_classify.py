#!/usr/bin/env python3
"""git_classify.py runs nothing that a repository's own config names.

Each test builds a repository whose config names a program for git to run
during git_classify.py's reads, checks that git_classify.py leaves it unrun,
then makes the same read with plain git to show the program was there to run.

  python3 tests/git_classify.py [-v]
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

SCRIPT = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "skills", "envrelay", "scripts", "git_classify.py"
)


class RepositoryHelpers(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = os.path.realpath(tmp.name)
        self.marks = os.path.join(self.root, "marks")
        self.global_config = os.path.join(self.root, "gitconfig")
        with open(self.global_config, "w") as f:
            f.write('[init]\n\tdefaultBranch = main\n[protocol "file"]\n\tallow = always\n')
        self.env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        self.env.update(
            HOME=self.root,
            GIT_CONFIG_GLOBAL=self.global_config,
            GIT_CONFIG_NOSYSTEM="1",
            GIT_AUTHOR_NAME="test",
            GIT_AUTHOR_EMAIL="test@example.com",
            GIT_COMMITTER_NAME="test",
            GIT_COMMITTER_EMAIL="test@example.com",
        )
        # Each records its name in self.marks when it runs. The filter passes
        # the content through, as a clean filter must.
        self.filter = self.program("filter", "cat")
        self.hook = self.program("hook", "exit 1")
        self.mtime = 1_600_000_000

    def program(self, name, tail):
        os.makedirs(os.path.join(self.root, "programs"), exist_ok=True)
        path = os.path.join(self.root, "programs", name)
        with open(path, "w") as f:
            f.write(f'#!/bin/sh\necho {name} >> "{self.marks}"\n{tail}\n')
        os.chmod(path, 0o755)
        return path

    def run_quiet(self, *argv):
        return subprocess.run(
            argv, env=self.env, stdin=subprocess.DEVNULL, capture_output=True, text=True, check=True
        ).stdout

    def git(self, repo, *argv):
        return self.run_quiet("git", "-C", repo, *argv)

    def repo(self, name, files):
        path = os.path.join(self.root, name)
        self.run_quiet("git", "init", "-q", path)
        for file, content in files.items():
            with open(os.path.join(path, file), "w") as f:
                f.write(content)
        self.git(path, "add", ".")
        self.git(path, "commit", "-qm", "files")
        return path

    def touch(self, path):
        """Changes the mtime but not the size, so git status reads the file again."""
        self.mtime += 60
        os.utime(path, (self.mtime, self.mtime))

    def ran(self):
        try:
            with open(self.marks) as f:
                names = f.read().split()
        except FileNotFoundError:
            return []
        os.remove(self.marks)
        return names

    def classify(self, path):
        out = self.run_quiet(sys.executable, SCRIPT, path)
        (report,) = json.loads(out)["repos"]
        return report

    def assert_not_run(self, repo, plain, touch=None):
        """git_classify.py runs nothing; plain git, making the same read, does."""
        if touch:
            self.touch(touch)
        report = self.classify(repo)
        self.assertEqual(self.ran(), [], "git_classify.py ran a program the repository named")
        if touch:
            self.touch(touch)
        self.git(repo, *plain)
        self.assertNotEqual(self.ran(), [], "plain git did not run it either, so this proves nothing")
        return report

    def test_fsmonitor(self):
        repo = self.repo("fsmonitor", {"a": "a\n"})
        self.git(repo, "config", "core.fsmonitor", self.hook)
        report = self.assert_not_run(repo, ["status", "--porcelain"])
        self.assertEqual(report["state"], "complete-repository")

    def test_filter_driver(self):
        repo = self.repo("filter", {".gitattributes": "* filter=own\n", "a": "a\n"})
        self.git(repo, "config", "filter.own.clean", self.filter)
        self.git(repo, "config", "filter.own.required", "true")
        report = self.assert_not_run(repo, ["status", "--porcelain"], touch=os.path.join(repo, "a"))
        self.assertEqual((report["state"], report["uncommitted"]), ("complete-repository", 0))

    def test_filter_driver_named_with_equals(self):
        # git -c would split "filter.a=b.clean=" at the first "=".
        repo = self.repo("equals", {".gitattributes": "* filter=a=b\n", "a": "a\n"})
        self.git(repo, "config", "filter.a=b.clean", self.filter)
        report = self.assert_not_run(repo, ["status", "--porcelain"], touch=os.path.join(repo, "a"))
        self.assertEqual(report["state"], "complete-repository")

    def test_filter_driver_in_a_submodule(self):
        source = self.repo("source", {".gitattributes": "* filter=own\n", "a": "a\n"})
        repo = self.repo("outer", {"b": "b\n"})
        self.git(repo, "submodule", "add", "-q", source, "sub")
        self.git(repo, "commit", "-qm", "sub")
        sub = os.path.join(repo, "sub")
        self.git(sub, "config", "filter.own.clean", self.filter)
        self.git(sub, "config", "filter.own.required", "true")
        report = self.assert_not_run(repo, ["status", "--porcelain"], touch=os.path.join(sub, "a"))
        self.assertEqual((report["state"], report["uncommitted"]), ("complete-repository", 0))

    def test_signed_stash(self):
        repo = self.repo("stash", {"a": "a\n"})
        tree = self.git(repo, "rev-parse", "HEAD^{tree}").strip()
        head = self.git(repo, "rev-parse", "HEAD").strip()
        commit = (
            f"tree {tree}\nparent {head}\n"
            "author test <test@example.com> 1600000000 +0000\n"
            "committer test <test@example.com> 1600000000 +0000\n"
            "gpgsig -----BEGIN PGP SIGNATURE-----\n \n iQEz\n -----END PGP SIGNATURE-----\n"
            "\nWIP on main\n"
        )
        stash = subprocess.run(
            ["git", "-C", repo, "hash-object", "-t", "commit", "-w", "--stdin"],
            env=self.env, input=commit, capture_output=True, text=True, check=True,
        ).stdout.strip()
        self.git(repo, "update-ref", "--create-reflog", "-m", "WIP on main", "refs/stash", stash)
        self.git(repo, "config", "log.showSignature", "true")
        self.git(repo, "config", "gpg.program", self.hook)
        report = self.assert_not_run(repo, ["stash", "list"])
        self.assertEqual((report["state"], report["stashes"]), ("complete-repository", 1))

    def test_filter_driver_from_the_users_config_still_runs(self):
        # The git lfs case: the user's own driver, used by the repository.
        with open(self.global_config, "a") as f:
            f.write(f'[filter "users"]\n\tclean = {self.filter}\n')
        repo = self.repo("users", {".gitattributes": "* filter=users\n", "a": "a\n"})
        self.ran()
        self.touch(os.path.join(repo, "a"))
        report = self.classify(repo)
        self.assertIn("filter", self.ran())
        self.assertEqual((report["state"], report["uncommitted"]), ("complete-repository", 0))

    def test_git_older_than_2_31(self):
        # Such a git has no GIT_CONFIG_COUNT, so none of the settings would
        # reach it; this one also has no --show-scope, which arrived in 2.26.
        repo = self.repo("old", {"a": "a\n"})
        self.git(repo, "config", "core.fsmonitor", self.hook)
        bin_dir = os.path.join(self.root, "bin")
        os.mkdir(bin_dir)
        with open(os.path.join(bin_dir, "git"), "w") as f:
            f.write(
                "#!/bin/sh\n"
                'case "$*" in *version*) echo "git version 2.24.3 (Apple Git-128)"; exit 0;; esac\n'
                "unset GIT_CONFIG_COUNT\n"
                f'exec "{shutil.which("git")}" "$@"\n'
            )
        os.chmod(os.path.join(bin_dir, "git"), 0o755)
        self.env["PATH"] = bin_dir + os.pathsep + self.env["PATH"]
        report = self.classify(repo)
        self.assertEqual(self.ran(), [], "git_classify.py ran a program the repository named")
        self.assertEqual(report["state"], "unknown")
        self.assertEqual(report["detail"], "git version 2.24.3 (Apple Git-128): git 2.31 or newer is needed")


if __name__ == "__main__":
    unittest.main()
