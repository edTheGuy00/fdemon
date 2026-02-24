## Task: Set Up git-cliff and Release Changelog Integration

**Objective**: Configure `git-cliff` for automated changelog generation from conventional commits, generate the initial `CHANGELOG.md`, and integrate changelog generation into the existing `release.yml` GitHub Actions workflow.

**Depends on**: None

### Scope

- `cliff.toml` — **NEW** git-cliff configuration at workspace root
- `CHANGELOG.md` — **NEW** generated changelog at workspace root
- `.github/workflows/release.yml` — Add changelog generation step

### Details

The project already uses conventional commits (`feat:`, `fix:`, `chore:`, `refactor:`, etc.) based on the git log. `git-cliff` will parse these commits and generate structured changelog entries grouped by type.

#### 1. Create `cliff.toml`

Place at workspace root (`/Users/ed/Dev/zabin/flutter-demon/cliff.toml`).

```toml
[changelog]
header = """
# Changelog

All notable changes to Flutter Demon are documented here.\n
"""
body = """
{% if version %}\
## [{{ version | trim_start_matches(pat="v") }}] - {{ timestamp | date(format="%Y-%m-%d") }}
{% else %}\
## [Unreleased]
{% endif %}\
{% for group, commits in commits | group_by(attribute="group") %}
### {{ group | striptags | trim | upper_first }}
{% for commit in commits %}\
- {% if commit.scope %}*({{ commit.scope }})* {% endif %}\
{% if commit.breaking %}[**breaking**] {% endif %}\
{{ commit.message | upper_first }}\
{% endfor %}
{% endfor %}\n
"""
footer = ""
trim = true

[git]
conventional_commits = true
filter_unconventional = true
split_commits = false
commit_parsers = [
    { message = "^feat", group = "Features" },
    { message = "^fix", group = "Bug Fixes" },
    { message = "^doc", group = "Documentation" },
    { message = "^perf", group = "Performance" },
    { message = "^refactor", group = "Refactoring" },
    { message = "^style", group = "Styling" },
    { message = "^test", group = "Testing" },
    { message = "^chore\\(release\\)", skip = true },
    { message = "^chore\\(deps\\)", skip = true },
    { message = "^chore|^ci", group = "Miscellaneous" },
    { body = ".*security", group = "Security" },
    { message = "^revert", group = "Reverted" },
]
protect_breaking_commits = false
filter_commits = false
tag_pattern = "v[0-9].*"
sort_commits = "oldest"
```

#### 2. Generate initial CHANGELOG.md

Run `git-cliff -o CHANGELOG.md` locally to generate the initial changelog from existing git history. This will be committed to the repository.

#### 3. Update `release.yml` to generate changelog

Add two capabilities to the existing release workflow:

**a) Generate release notes from changelog:**

In the `release` job, after checking out the repository, add a step that generates the changelog for the latest tag only (for GitHub Release body):

```yaml
- name: Checkout
  uses: actions/checkout@v4
  with:
    fetch-depth: 0   # Required for git-cliff to access full history

- name: Generate release changelog
  uses: orhun/git-cliff-action@v4
  id: changelog
  with:
    config: cliff.toml
    args: --latest --strip header
  env:
    OUTPUT: CHANGES.md
```

**b) Use generated changelog as release body:**

Update the `softprops/action-gh-release@v2` step to use the generated changelog instead of `generate_release_notes: true`:

```yaml
- name: Create GitHub Release
  uses: softprops/action-gh-release@v2
  with:
    draft: false
    prerelease: false
    body_path: CHANGES.md
    files: |
      artifacts/fdemon-v*.tar.gz
      artifacts/fdemon-v*.zip
      artifacts/checksums-sha256.txt
```

**c) Update CHANGELOG.md in repo (optional, can be manual):**

After the release, the full CHANGELOG.md should be regenerated and committed. This can be done manually (`git-cliff -o CHANGELOG.md && git commit`) or via a post-release workflow step. For now, document the manual process — automation can be added later.

### Acceptance Criteria

1. `cliff.toml` exists at workspace root with conventional commit parsers
2. `CHANGELOG.md` is generated and committed with entries from existing git history
3. `release.yml` uses `orhun/git-cliff-action@v4` to generate per-release changelog
4. GitHub Releases use the generated changelog body instead of auto-generated notes
5. The `release` job checkout step includes `fetch-depth: 0`
6. The workflow is valid YAML (lint with `actionlint` or manual review)

### Testing

- Run `git-cliff --dry-run` locally to verify the config produces meaningful output
- Verify the `cliff.toml` parses correctly: `git-cliff --config cliff.toml -o /dev/null`
- Review the generated `CHANGELOG.md` for correct grouping and formatting
- Validate `release.yml` syntax

### Notes

- Install `git-cliff` locally: `cargo install git-cliff --locked`
- The `orhun/git-cliff-action@v4` action handles installation in CI automatically
- The existing `release.yml` uses `generate_release_notes: true` — this must be replaced with `body_path: CHANGES.md`
- The `fetch-depth: 0` is critical — git-cliff needs the full git history to resolve tags and commit ranges
- Ensure the `release` job in the existing workflow already has `fetch-depth: 0` or add it
