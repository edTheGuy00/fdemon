## Task: Pin GitHub Actions to commit SHAs

**Objective**: Harden CI workflows against tag mutation and supply-chain attacks. Currently `.github/workflows/ci.yml` (and sibling workflows) reference `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, and `Swatinem/rust-cache@v2` by mutable tag. A force-pushed or compromised tag could substitute malicious code into the action. Pinning to immutable commit SHAs eliminates this risk.

**Depends on**: Task 03 (which modifies `ci.yml`'s clippy step) — Wave C

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `.github/workflows/ci.yml`: pin all third-party actions to commit SHAs.
- `.github/workflows/e2e.yml` (if it exists): same.
- `.github/workflows/release.yml` (if it exists): same.

**Files Read (Dependencies):**
- The current state of each workflow file.
- The action repositories' release pages (e.g. `https://github.com/actions/checkout/releases`) to find the SHA corresponding to the tag we currently use.

### Details

#### Pinning pattern

Replace `uses: <owner>/<repo>@<tag>` with `uses: <owner>/<repo>@<sha> # <tag>`. The trailing comment preserves human readability and signals the intended version to Renovate/Dependabot.

Example:

```yaml
# Before
- uses: actions/checkout@v4

# After
- uses: actions/checkout@b4ffde65f46336ab88eb53be808477a3936bae11 # v4.1.1
```

#### Actions to pin

Identify every `uses:` line in each workflow file. Common ones in this project (verified at write time):

| Action | Tag in use | SHA (verify at write time) |
|--------|-----------|----------------------------|
| `actions/checkout` | `@v4` | look up the latest `v4.x` SHA |
| `dtolnay/rust-toolchain` | `@stable` | look up the latest commit (this action does not tag stable releases — pin to a specific commit) |
| `Swatinem/rust-cache` | `@v2` | look up the latest `v2.x` SHA |
| Any other `uses:` | varies | pin individually |

Use the command:

```bash
gh api repos/<owner>/<repo>/git/ref/tags/<tag> --jq .object.sha
# example:
gh api repos/actions/checkout/git/ref/tags/v4.1.1 --jq .object.sha
```

For `dtolnay/rust-toolchain@stable`, the `stable` ref is a branch, not a tag. Use:

```bash
gh api repos/dtolnay/rust-toolchain/branches/stable --jq .commit.sha
```

#### Renovate/Dependabot compatibility

Both Renovate and Dependabot support pinned SHAs with trailing version comments and will automatically open PRs when newer versions ship. No additional config needed if the repository already runs one of those tools; otherwise, this is a one-time pin and updates become manual.

### Acceptance Criteria

1. Every `uses:` line in `.github/workflows/*.yml` references an action by commit SHA.
2. Each pinned line carries a trailing comment naming the corresponding human-readable tag (e.g. `# v4.1.1`).
3. The workflows still execute correctly on the next CI push — verified post-merge by observing the next CI run.
4. No functional change to workflow logic — only `@<tag>` → `@<sha>` substitutions.

### Testing

```bash
# Verify the YAML is still parseable
ruby -ryaml -e "YAML.load_file('.github/workflows/ci.yml')"
ruby -ryaml -e "YAML.load_file('.github/workflows/e2e.yml')"   # if exists
ruby -ryaml -e "YAML.load_file('.github/workflows/release.yml')" # if exists

# Confirm SHAs resolve
gh api repos/actions/checkout/git/commits/<sha-from-ci-yml> --jq .sha
```

### Notes

- This task lands AFTER Task 03 because Task 03 also modifies `ci.yml`. The orchestrator must sequence them (Task 03 in Wave A, Task 08 in Wave C — confirmed in TASKS.md).
- If a workflow uses `actions/setup-*` or other actions not listed above, pin them too — apply the same pattern.
- Do NOT pin the `runs-on:` runner version (e.g. `ubuntu-latest`) — those are GitHub-managed VMs, not actions, and the pinning concern doesn't apply.
- `dtolnay/rust-toolchain` is special: the `stable` ref tracks the latest stable Rust release. Pinning to a SHA freezes the toolchain version, which is what we want for reproducibility but means manual bumps when new Rust stables ship. Document this tradeoff in a comment near the pinned line.
- If GitHub Actions ever introduces signed releases (already in beta for some actions), prefer those once they're GA.
