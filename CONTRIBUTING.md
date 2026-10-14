# Contributing to Satchel

This is how the three of us work so nobody steps on anybody's toes.

## 1. Pick up work

- All work is tracked as [issues](https://github.com/frectrium/Satchel/issues) on the [project board](https://github.com/users/frectrium/projects).
- Each issue has an owner, a milestone (the week it belongs to), and labels for its type and area.
- When you start an issue, move it to **In Progress** on the board. If you're blocked, say so in a comment on the issue.
- New work that comes out of research gets its own issue. Use the **Research task** or **Task** template.

## 2. Branch

Branch off `main` and name the branch `<type>/<issue-number>-<short-slug>`:

```
research/3-s1-filing
docs/13-architecture-diagram
feat/24-block-index
infra/21-terraform-ap-south-1
fix/30-node-health-port
```

## 3. Commit

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(node): store blocks on disk keyed by hash
docs(research): add S-1 cost figures
infra(terraform): add 3-AZ VPC for ap-south-1
fix(api): return 404 for unknown blocks
```

## 4. Open a pull request

- Put `Closes #<issue>` in the PR description so the issue closes when the PR merges.
- `main` is protected. Every change goes through a PR with **one approving review** and **green CI**.
- PRs are **squash-merged** and the branch is deleted automatically.
- Keep PRs small. A research PR that adds three sources and a page of notes is a good size.

## 5. Code checks

Run these before pushing. CI runs the same checks.

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 6. Research and writing

- Add every source to the [Source Log](docs/sources/README.md) and cite it by ID.
- Prefer primary sources: Dropbox's own blog posts, talks and filings. Media coverage is for context only.
- Any design decision in Satchel must point to the Magic Pocket decision it mirrors. Record it in the Design Decision Log.

## 7. Cloud costs

- Never commit AWS credentials. `.env` and `*.tfvars` are gitignored for this reason.
- Run `terraform destroy` after every experiment run. Post in the team chat when infrastructure is up and when it's down.
