---
name: pr
description: Builds the title and description of a ytglide pull request from the current branch, using .github/pull_request_template.md. Use when the user asks to "write the PR", "PR description" or /pr.
---

# Writing a PR

1. **Base.** If the current branch is `dev`, the base is `main`; otherwise it is `dev`. Confirm with `git branch --show-current`.
2. **Context.** Run `git log <base>..HEAD --format='%h %s'` and `git diff <base>...HEAD --stat`. Read the key files in the diff if the reason does not come out of the commits.
3. **Template.** Read `.github/pull_request_template.md` and fill in every section in English, without leaving the `<!-- -->` comments.
   - *What changes*: bullets per behaviour, not per file.
   - *Why*: the real motive; if you do not know it, ask instead of inventing it.
   - *How it was tested*: tick `[x]` only what you ran in this session and paste the real result (passed/failed). If you did not run it, leave it unticked.
   - *Notes for reviewers*: new config keys (with their README row), keybinding changes, and what was left out.
4. **Title.** One line in Conventional Commits style, in English (`fix: ...`, `feat: ...`, `chore: ...`).
5. **Delivery.** Show the title and body to the user. Do not push or run `gh pr create` unless asked explicitly; if asked: `gh pr create --base <base> --title "..." --body-file <file>`.
