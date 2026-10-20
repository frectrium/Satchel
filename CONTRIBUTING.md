# Contributing to Satchel

This is how the three of us work, from picking up an issue to getting it merged. If something here doesn't fit a real situation, raise it in an issue and we'll change this file.

## 1. Before you start

1. Set up the repo locally by following **Getting started** in the [README](README.md). This is issue [#1](https://github.com/frectrium/Satchel/issues/1).
2. Read the [project brief](docs/SATCHEL_PROJECT.md) and the plan for the current week (see section 3).
3. Make sure your git name and email match your GitHub account, so your commits are linked to you:
   ```bash
   git config user.name "Your Name"
   git config user.email "you@example.com"
   ```

## 2. How work flows (SDLC)

Every piece of work follows the same path. The issue for each stage already exists on the board.

| Stage | What happens | Where |
|---|---|---|
| Requirements | Scope, objectives and the weekly deliverables | [Project brief](docs/SATCHEL_PROJECT.md), [milestones](https://github.com/frectrium/Satchel/milestones) |
| Research | Primary sources, cited by Source Log ID | [Source Log](docs/sources/README.md), `docs/research/` |
| Design | Exercise plan, architecture, Design Decision Log, test strategy | `docs/design/` |
| Build | One issue per component, each with its own tests | `crates/` |
| Test | Unit tests, integration tests against the local cluster, CI on every PR | `cargo test`, CI |
| Deploy | AWS infrastructure as code with Terraform, torn down after each run | `infra/` |
| Measure | Experiments, metrics and dashboards, cost models | `docs/experiments/`, `docs/cost/` |
| Release | Tagged release that the final report and viva use | GitHub Releases |

## 3. Pick up an issue

### How the weeks are tracked

Everything lives on GitHub, not in the docs folder:

- **Milestones:** one per week, with its due date.
- **Deliverable issues:** each week's deliverable issue (for example [#10](https://github.com/frectrium/Satchel/issues/10) for Week 1) holds the plan for the week: the goal, the order of work and what done means. That week's tasks are its **sub-issues**, so its progress bar shows how far along the week is.
- **[Project board](https://github.com/users/frectrium/projects/1):** the **Current week** view is the kanban for this week, **By week** shows every week, and **By person** shows who owns what.
- **Status updates:** a short update is posted on the project at the start of each week.

### Your issues

- All work is tracked as [issues](https://github.com/frectrium/Satchel/issues) on the [project board](https://github.com/users/frectrium/projects/1).
- Each issue has an owner, a milestone (the week it belongs to) and labels for its type and area. Work on issues assigned to you. If you want to take someone else's, ask them first.
- When you start, move the issue to **In Progress** on the board.
- If you're blocked, comment on the issue saying what you're waiting for and add the `blocked` label.
- New work that comes up gets its own issue. Use the **Research task**, **Task** or **Bug** template, give it the week's milestone, and add it as a sub-issue of that week's deliverable issue.

## 4. Create a branch

Never commit to `main` directly. It's protected and will reject the push.

Start from an up-to-date `main` and name the branch `<type>/<issue-number>-<short-slug>`:

```bash
git switch main
git pull
git switch -c research/4-s1-filing
```

| Type | Use it for | Example |
|---|---|---|
| `research` | Finding and summarising sources | `research/4-s1-filing` |
| `docs` | Reports, design docs, README | `docs/11-architecture-diagram` |
| `feat` | New Satchel functionality | `feat/40-block-index` |
| `fix` | Bug fixes | `fix/41-node-health-port` |
| `test` | Tests and test tooling only | `test/32-compose-smoke-test` |
| `infra` | Terraform, Docker, CI | `infra/20-terraform-ap-south-1` |
| `chore` | Dependencies and housekeeping | `chore/45-bump-axum` |

One branch per issue. Keep the slug short and lowercase, with hyphens.

## 5. Commit

Use [Conventional Commits](https://www.conventionalcommits.org/): `<type>(<scope>): <what changed>`, in the imperative.

```
feat(node): store blocks on disk keyed by hash
test(api): cover 404 for unknown blocks
docs(research): add S-1 cost figures
infra(terraform): add 3-AZ VPC for ap-south-1
fix(api): return 404 for unknown blocks
```

Commit as often as you like on your branch. Only the PR title ends up on `main` (see section 8).

## 6. Test

Every code change comes with tests. Docs-only changes don't need any.

- **Unit tests** go next to the code, in a `#[cfg(test)] mod tests` block.
- **Integration tests** that cross services run against the local Docker Compose cluster. The harness is being set up in [#32](https://github.com/frectrium/Satchel/issues/32). The [test strategy](https://github.com/frectrium/Satchel/issues/31) covers what each component needs.
- A bug fix comes with a test that fails without the fix.

Before you push, run the same checks CI runs:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
docker compose up --build    # then check: curl localhost:8080/health
```

## 7. Keep your branch up to date

If `main` moves while you're working, bring your branch up to date before asking for review:

```bash
git fetch origin
git rebase origin/main
git push --force-with-lease
```

Force-pushing is fine on your own branch, never on `main`. You can also use the **Update branch** button on the PR.

## 8. Open a pull request

1. Push your branch: `git push -u origin <branch>`.
2. Open a PR against `main`. The template fills in the description.
3. **Write the PR title as a Conventional Commit**, for example `docs(research): add S-1 cost figures`. We squash-merge, so the PR title becomes the commit on `main`.
4. Put `Closes #<issue>` in the description, so the issue closes and moves to **Done** when the PR merges.
5. Open it as a **draft** if it's not ready yet, and mark it ready when it is.
6. Everyone is requested as a reviewer automatically (CODEOWNERS).

Keep PRs small. A research PR that adds three sources and a page of notes is a good size.

## 9. Review and merge

`main` is protected by a ruleset. A PR can merge only when:

| Rule | What it means |
|---|---|
| 1 approval | Someone other than the author approves. A new push dismisses earlier approvals |
| Conversations resolved | Every review comment is resolved |
| CI green | `Rust (fmt, clippy, test)` and `Docker image builds` both pass |
| Squash merge only | The PR becomes one commit on `main`, and the branch is deleted |
| No force-push or deletion | `main`'s history can't be rewritten |

When you review, check that:

- it does what the issue asks, and nothing else
- code has tests, and they test the behaviour rather than the implementation
- every claim in research and docs is cited by Source Log ID
- design choices are in the Design Decision Log with their Magic Pocket counterpart

The author merges once it's approved and green.

## 10. Definition of done

An issue is done when:

- [ ] Its checklist is complete, or the PR explains anything left out
- [ ] Code has tests, and fmt, clippy and test pass in CI
- [ ] Sources are in the Source Log and claims are cited
- [ ] Design decisions are recorded in the Design Decision Log
- [ ] README and docs are updated if setup or behaviour changed
- [ ] The PR is approved, merged, and the issue is closed

## 11. Research and writing

- Add every source to the [Source Log](docs/sources/README.md) and cite it by ID, for example `[B2]`.
- Prefer primary sources: Dropbox's own blog posts, talks and filings. Media coverage is for context only.
- Separate what a source says from our interpretation. Mark inferences with *(our inference)*.
- Any design decision in Satchel must point to the Magic Pocket decision it mirrors. Record it in the Design Decision Log.

## 12. Cloud and costs

- All AWS infrastructure is created by Terraform in `infra/`. Don't create resources by hand in the console.
- Use AWS CLI profiles or SSO for credentials. Never commit keys. `.env`, `*.tfvars`, `*.pem` and `*.key` are gitignored for this reason.
- Run `terraform destroy` after every experiment run. Post in the team chat when infrastructure goes up and when it comes down.
