# Plan: Additional Installation Methods (Homebrew, AUR) + install.sh 403 Fix

## TL;DR

Diagnose and fix the `curl | bash` 403 reported in [#41](https://github.com/edTheGuy00/fdemon/issues/41) (almost certainly GitHub REST API rate-limiting on `api.github.com/repos/.../releases/latest`) by switching version resolution to the unauthenticated `github.com/.../releases/latest` redirect with the API as a fallback. Then add two community-style distribution channels: a **Homebrew tap** (`edTheGuy00/homebrew-tap`) and an **Arch User Repository** package (`fdemon-bin`). Both consume the existing pre-built archives produced by `release.yml`, and both are auto-bumped from the release pipeline on every tag, so day-to-day release work doesn't change.

---

## Background

Today, `fdemon` is distributed exclusively via:

1. A curl-piped shell script (`install.sh`) that downloads pre-built `tar.gz`/`zip` artifacts from GitHub Releases.
2. Building from source with `cargo install --path crates/flutter-demon` (undocumented in the README).

Two open issues request improvements:

- **[#41 — curl error on linux](https://github.com/edTheGuy00/fdemon/issues/41)** (CachyOS): the install script fails. Title says "curl error"; screenshot (per issue) shows a 403. The only outbound call in `install.sh` before download is `curl -fsSL https://api.github.com/repos/edTheGuy00/fdemon/releases/latest`. That endpoint is **rate-limited to 60 requests/hour per source IPv4 for unauthenticated callers** ([GitHub REST primary rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api#primary-rate-limit-for-unauthenticated-users)). On shared NAT, VPN, mobile data, or popular Arch derivatives (CachyOS users often share carrier-grade-NAT pools), this is the most common failure mode. There is also a "secondary rate limit" (abuse detection) that returns 403 with a different body; behavior at the script level is identical.

- **[#40 — Homebrew installation](https://github.com/edTheGuy00/fdemon/issues/40)**: macOS/Linux users want `brew install fdemon`. The user offered to contribute a PR.

A natural third addition — **AUR** — both serves the original CachyOS user (CachyOS is Arch-based) and is cheap to add since we already produce stripped `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` tarballs.

Across the three additions, the *only* new artifact our release pipeline needs to produce is a stable, machine-readable SHA256 manifest — which `release.yml` already emits as `checksums-sha256.txt`. Everything else is metadata in external repos.

---

## Diagnosis: Why the 403?

Read of `install.sh` (lines 132–136) shows the script's only API call:

```bash
get_latest_version() {
    curl -fsSL "$GITHUB_API" \
        | grep '"tag_name"' \
        | sed -E 's/.*"v([^"]+)".*/\1/'
}
```

Failure modes that return HTTP 403 from `api.github.com`:

| # | Cause | Likelihood for CachyOS user | Fix |
|---|-------|----------------------------|-----|
| 1 | **Primary rate limit** — 60 unauth req/hr/IPv4 exhausted by shared NAT, VPN, mobile carrier, or re-running the script. | **High** — most likely. | Stop using `api.github.com`; use the `github.com` redirect. |
| 2 | **Secondary rate limit / abuse detection** — bursty automated calls trigger a temporary block. | Medium. | Same fix; backoff retry as defensive layer. |
| 3 | **Missing `User-Agent` header** — GitHub API rejects requests without UA. `curl` sets one by default (`curl/X.Y`), so this is rare unless a system curl config strips it. | Low. | Explicitly pass `-A "fdemon-install"`. |
| 4 | **Geo/ASN-level filtering** by GitHub's edge — uncommon, occasional reports against specific ASNs. | Low. | Fallback path via plain HTML redirect. |
| 5 | **Corporate proxy / MITM** — proxy rewrites 403. | Low for a personal CachyOS install. | Document `https_proxy` env var. |

`set -euo pipefail` + `curl -fsSL` makes the script exit on the first 403 without further attempts. The script then prints `Failed to resolve latest version from GitHub API`, but the user sees the raw curl error first because `-f` causes curl itself to print "The requested URL returned error: 403" before exit.

**The fix is structural, not a workaround**: avoid the API endpoint entirely. `https://github.com/{owner}/{repo}/releases/latest` returns a `302 Location: /{owner}/{repo}/releases/tag/v0.5.0` which is **not subject to the REST API rate limit** (it's served from the regular github.com edge). Parsing the version out of the `Location` header is trivial and depends on no JSON.

Recommended resolution order in the new script:

1. **Primary**: `curl -fsSLI -o /dev/null -w '%{url_effective}' https://github.com/edTheGuy00/fdemon/releases/latest` → parse the trailing `vX.Y.Z`.
2. **Fallback**: existing API call (preserves explicit JSON error responses when GitHub is broader-broken).
3. **Last resort**: print actionable error pointing the user at `--version X.Y.Z` and the release page.

This eliminates the failure mode permanently for unauthenticated users on rate-limited IPs.

---

## Affected Modules

- `install.sh` — **MODIFY** Replace `get_latest_version` with redirect-based resolution; add API as fallback; add `User-Agent`. (~30 LOC change.)
- `.github/workflows/release.yml` — **MODIFY** Add two new jobs after `release` succeeds: `bump-homebrew-tap` and `publish-aur`. Both run only on successful publish, both fail-soft so a tap/AUR outage doesn't block the GitHub release.
- `README.md` — **MODIFY** Add `brew install edTheGuy00/tap/fdemon` and `yay -S fdemon-bin` (or `paru`) to the Installation section.
- `docs/DEVELOPMENT.md` — **MODIFY** Add a "Distribution channels" subsection documenting the tap repo location, AUR package name, and how to manually re-publish if automation fails. *(Routed to `doc_maintainer` agent.)*
- **NEW repo: `edTheGuy00/homebrew-tap`** — Hosts `Formula/fdemon.rb`. Lives outside this repo. PR-driven by automation.
- **NEW AUR package: `fdemon-bin`** — Hosted at `ssh+git://aur@aur.archlinux.org/fdemon-bin.git`. PKGBUILD + `.SRCINFO`. Lives outside this repo.

Nothing in `crates/` changes. No Rust code or tests are affected.

---

## Development Phases

### Phase 1: Fix install.sh 403 (bug — high priority)

**Goal**: Make `curl | bash` reliable on rate-limited or shared-NAT networks (CachyOS, mobile data, VPNs).

**Duration**: 1–2 hours including manual smoke tests.

#### Steps

1. **Rewrite `get_latest_version`** in `install.sh`.
   - Use `curl -fsSLI -A "fdemon-install" --retry 2 --retry-delay 1 -o /dev/null -w '%{url_effective}\n' https://github.com/${REPO}/releases/latest` to follow the 302 and capture the final URL.
   - Extract the tag with `awk -F/ '{print $NF}'` and strip the leading `v`.
   - If the redirect resolution fails (e.g., returns the same URL or empty), fall back to the existing API call, also with `-A` and `--retry 2`.
   - If both fail, surface a clearer message: include the offending URL, hint at `--version`, and link to releases.

2. **Add a `User-Agent` everywhere `curl` is invoked** — defensive against future GitHub policy tightening. Single constant near top of script: `USER_AGENT="fdemon-install (https://github.com/${REPO})"`.

3. **Reproduce-and-test loop**:
   - On a clean Linux box (or container) with networking that mimics rate-limit exhaustion: pre-burn the 60 req/hr by curling the API in a loop, then run the script and confirm it now succeeds.
   - On a normal connection: run with and without `--version` to confirm both paths still work.
   - On macOS arm64: smoke test against `v0.5.0`.

**Milestone**: A CachyOS user (or anyone on rate-limited shared IP) can `curl | bash` and the script resolves the latest version successfully.

---

### Phase 2: Homebrew tap

**Goal**: Enable `brew install edTheGuy00/tap/fdemon` on macOS (x86_64 + arm64) and Linuxbrew (x86_64 + aarch64). Auto-bump on every release.

**Duration**: 3–4 hours for first formula + automation; ~30 min/release thereafter (fully automated).

**Decision: binary formula, not source.** A source formula would require `depends_on "rust" => :build`, which means a multi-minute `cargo build --release` on each user's machine. Since we already publish stripped, optimized binaries for all four target triples Homebrew cares about, a binary formula is simpler, faster for users, and keeps build determinism under our control.

#### Steps

1. **Create `edTheGuy00/homebrew-tap` repository** (manual, one-time).
   - GitHub naming convention: `homebrew-<tapname>` so that `brew tap edTheGuy00/tap` resolves automatically. Use `homebrew-tap`.
   - Add a `README.md`, `LICENSE`, and `Formula/` directory.
   - No CI needed initially; `brew test-bot` can be added later if the formula gets complex.

2. **Author `Formula/fdemon.rb`** in that repo. Skeleton:

   ```ruby
   class Fdemon < Formula
     desc "High-performance TUI for Flutter development"
     homepage "https://fdemon.dev"
     version "0.5.0"
     license "BUSL-1.1"

     on_macos do
       on_arm do
         url "https://github.com/edTheGuy00/fdemon/releases/download/v#{version}/fdemon-v#{version}-aarch64-apple-darwin.tar.gz"
         sha256 "<sha256>"
       end
       on_intel do
         url "https://github.com/edTheGuy00/fdemon/releases/download/v#{version}/fdemon-v#{version}-x86_64-apple-darwin.tar.gz"
         sha256 "<sha256>"
       end
     end

     on_linux do
       on_arm do
         url "https://github.com/edTheGuy00/fdemon/releases/download/v#{version}/fdemon-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
         sha256 "<sha256>"
       end
       on_intel do
         url "https://github.com/edTheGuy00/fdemon/releases/download/v#{version}/fdemon-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
         sha256 "<sha256>"
       end
     end

     def install
       bin.install "fdemon"
     end

     test do
       assert_match version.to_s, shell_output("#{bin}/fdemon --version")
     end
   end
   ```

   - License field: Homebrew uses [SPDX identifiers](https://spdx.org/licenses/); BSL-1.1 is `BUSL-1.1` in SPDX. Homebrew core wouldn't accept this (non-OSI), but **personal taps have no license restrictions**.

3. **Wire automation in `release.yml`**: add a new job `bump-homebrew-tap` that runs after `release` succeeds.
   - Uses a fine-grained PAT (stored as `HOMEBREW_TAP_TOKEN`) with `contents: write` scope on the tap repo only — *not* the existing release GitHub App, which is scoped to the main repo.
   - Approach A (recommended): use `dawidd6/action-homebrew-bump-formula@v5` — handles SHA256 computation and PR/commit creation. Maintained, widely used.
   - Approach B (manual): a small shell step that `gh repo clone`s the tap, computes SHAs from `checksums-sha256.txt`, runs `sed`, commits, and pushes.
   - Either way: read SHA256 values from the already-uploaded `checksums-sha256.txt` artifact rather than re-hashing.
   - Set `continue-on-error: true` so a tap-bump failure doesn't break the release (it's downstream of the actual binary publish).

4. **Update `README.md` Installation section** to show:
   ```bash
   brew install edTheGuy00/tap/fdemon
   ```
   as the preferred macOS/Linuxbrew path, with the curl|bash retained as fallback.

5. **Smoke test**: `brew tap edTheGuy00/tap && brew install fdemon && fdemon --version` on a clean macOS arm64 box.

**Milestone**: `brew install edTheGuy00/tap/fdemon` works for all four supported (macOS/Linux × arm/intel) targets, and every future GitHub release automatically updates the formula within minutes.

---

### Phase 3: Arch User Repository (`fdemon-bin`)

**Goal**: Enable `paru -S fdemon-bin` / `yay -S fdemon-bin` on Arch and Arch derivatives (Manjaro, EndeavourOS, **CachyOS**). Auto-bump on every release.

**Duration**: 3–4 hours for first PKGBUILD + automation; ~15 min/release thereafter (fully automated).

**Decision: ship a `-bin` package only, initially.** A `fdemon` source package would invoke `cargo build --release` at install time and require `rust` as a `makedepends`. A `fdemon-bin` package downloads our pre-built `tar.gz` and is ready in seconds. We can add the source variant later if there's demand; the `-bin` variant alone covers the CachyOS user from issue #41.

#### Steps

1. **Create an AUR account** (manual, one-time).
   - Register at https://aur.archlinux.org with the `edTheGuy00` identity.
   - Add an SSH public key (generate a new dedicated ed25519 keypair; keep the private key in a CI secret called `AUR_SSH_KEY`).
   - Verify push access: `ssh aur@aur.archlinux.org help`.

2. **Author `PKGBUILD`** and `.SRCINFO` for `fdemon-bin`. Skeleton:

   ```bash
   # Maintainer: ed <claude_delta@thxlab.io>
   pkgname=fdemon-bin
   pkgver=0.5.0
   pkgrel=1
   pkgdesc="High-performance TUI for Flutter development (pre-built binary)"
   arch=('x86_64' 'aarch64')
   url="https://github.com/edTheGuy00/fdemon"
   license=('custom:BUSL-1.1')
   provides=('fdemon')
   conflicts=('fdemon')
   source_x86_64=("https://github.com/edTheGuy00/fdemon/releases/download/v${pkgver}/fdemon-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz")
   source_aarch64=("https://github.com/edTheGuy00/fdemon/releases/download/v${pkgver}/fdemon-v${pkgver}-aarch64-unknown-linux-gnu.tar.gz")
   sha256sums_x86_64=('<sha256>')
   sha256sums_aarch64=('<sha256>')

   package() {
       install -Dm755 "${srcdir}/fdemon" "${pkgdir}/usr/bin/fdemon"
   }
   ```

   - `provides`/`conflicts` declare `fdemon` so future source and `-git` packages slot cleanly.
   - License field: Arch packaging guidelines allow non-OSI licenses via `custom:<name>`. The full BSL-1.1 text should be installed to `/usr/share/licenses/fdemon-bin/LICENSE` (we can add this step in `package()` once we wire up the LICENSE artifact — see Risks).

3. **Wire automation in `release.yml`**: add a new job `publish-aur` that runs after `release` succeeds.
   - Uses `KSXGitHub/github-actions-deploy-aur@v4` — the most-maintained AUR push action. Computes `.SRCINFO`, commits, pushes via SSH.
   - Needs three secrets: `AUR_SSH_KEY` (private), `AUR_USERNAME`, `AUR_EMAIL`. Document in `DEVELOPMENT.md`.
   - Input: a templated PKGBUILD with `pkgver=__VERSION__` placeholders that a prior step substitutes from `needs.version.outputs.version` plus the SHA256s read from `checksums-sha256.txt`.
   - `continue-on-error: true` — same reasoning as the Homebrew bump job.

4. **Update `README.md`** to show:
   ```bash
   # AUR (Arch, CachyOS, EndeavourOS, Manjaro)
   paru -S fdemon-bin
   ```

5. **Smoke test**: on a CachyOS or Arch container, `makepkg -si` from a local checkout of the PKGBUILD before pushing to AUR; then `paru -S fdemon-bin` once published.

**Milestone**: Arch users (including CachyOS) can install via their preferred AUR helper. Every release auto-publishes a new PKGBUILD.

---

### Phase 4: Documentation & rollout

**Goal**: Update user-facing and contributor-facing docs to reflect the new channels, and document the manual override path if automation breaks.

**Duration**: 1 hour.

#### Steps

1. **Update `README.md`** — Installation section gets a three-option ladder (Homebrew, AUR, curl-pipe-bash), with a one-line "Build from source" pointer at the bottom.
2. **Update `docs/DEVELOPMENT.md`** *(via `doc_maintainer` agent)* — Add a "Distribution Channels" subsection covering:
   - Tap repo location and how to manually edit the formula in an emergency.
   - AUR package name and `ssh+git://` clone URL.
   - Which secrets are required in this repo's settings.
   - How to manually re-trigger a bump for a given tag.
3. **Update `fdemon.dev` install page** *(separate repo if applicable — out of scope for this plan, but flag it)*.

**Milestone**: A new contributor can read `DEVELOPMENT.md` and re-publish any channel by hand without reverse-engineering the workflow.

---

## Edge Cases & Risks

### Cache invalidation on the GitHub `releases/latest` redirect

- **Risk:** GitHub caches the `releases/latest` redirect briefly (typically <60 s). For a user who runs `curl | bash` the *instant* a new release publishes, the redirect could still point at the previous tag.
- **Mitigation:** Acceptable in practice — the script also accepts `--version`, and the window is tiny. Document the workaround for power users.

### Rate-limit fallback going stale

- **Risk:** If we silently fall back to `api.github.com` and it 403s too, the user sees a confusing two-step error.
- **Mitigation:** When both paths fail, print a single consolidated error that names *both* attempted URLs and points at `--version` and the release page.

### Homebrew tap PR loop / merge conflicts

- **Risk:** Concurrent releases (rare for this project but possible during a hotfix burst) could collide on the formula bump.
- **Mitigation:** The bump action is idempotent on `version`; if two runs try to bump to the same version, the second is a no-op. Cross-version collisions resolve by re-running. Keep `continue-on-error: true`.

### AUR PKGBUILD checksum mismatch

- **Risk:** If `checksums-sha256.txt` formatting changes (e.g., line-ending shifts), the substitution step in the publish-aur job breaks.
- **Mitigation:** Pin a parser script (awk/grep) and unit-test it once with a shellcheck pass. Keep a manual override doc.

### License text on AUR

- **Risk:** Arch packaging convention requires non-OSI licenses to install a copy of the LICENSE file. We currently don't ship `LICENSE` in the release tarballs.
- **Mitigation:** Either (a) add `LICENSE` to the tarball in `release.yml`'s Package step (small change, ~3 LOC); or (b) have the PKGBUILD fetch LICENSE directly from `raw.githubusercontent.com` as a second `source=`. Prefer (a) — cleaner.

### `set -euo pipefail` masking `curl` retry behavior

- **Risk:** `--retry 2 --retry-delay 1` only retries on transient network errors, *not* HTTP 4xx. So adding `--retry` doesn't help with 403; the redirect fallback is what matters.
- **Mitigation:** Test path with deliberate API rate-limit exhaustion. Don't depend on `--retry` for 403 mitigation.

### CachyOS reporter wants the curl-pipe-bash fix, not just a new package

- **Risk:** The user in #41 may not have an AUR helper installed (unlikely on CachyOS, but possible). If we close #41 by saying "use the AUR package", we'd be ignoring the script bug.
- **Mitigation:** Phase 1 ships first and stands alone. Phases 2–3 are additive.

### License compatibility for Homebrew

- **Risk:** Homebrew's `homebrew-core` formula repo requires OSI-approved licenses; BSL-1.1 is not OSI. We could not push `fdemon` to `homebrew-core`.
- **Mitigation:** None needed — a personal tap (`edTheGuy00/homebrew-tap`) has no such restriction. Document that we won't pursue homebrew-core unless we relicense.

### Secret scope

- **Risk:** Granting a token broad access to update external repos increases blast radius if compromised.
- **Mitigation:** Use a fine-grained PAT scoped to the tap repo only for `HOMEBREW_TAP_TOKEN`. AUR uses an SSH key, not a token, and its only authority is push to AUR packages owned by the registered user — limit that account's package list.

---

## Configuration Additions

No new project-level config files. Two new GitHub Actions secrets/vars in this repo:

| Name | Type | Scope | Purpose |
|------|------|-------|---------|
| `HOMEBREW_TAP_TOKEN` | Secret | repo | Fine-grained PAT, write access to `edTheGuy00/homebrew-tap` only. |
| `AUR_SSH_KEY` | Secret | repo | Private SSH key for the AUR account. |
| `AUR_USERNAME` | Variable | repo | AUR account name. |
| `AUR_EMAIL` | Variable | repo | AUR account email. |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `install.sh` resolves `latest` via the github.com redirect by default and falls back to the API only on failure.
- [ ] `User-Agent` header is sent on every `curl` invocation in the script.
- [ ] Manual reproduction with a rate-limit-exhausted IP completes installation successfully.
- [ ] Issue #41 is closed with a comment linking to the merged PR.

### Phase 2 Complete When:
- [ ] `edTheGuy00/homebrew-tap` exists and contains a working `Formula/fdemon.rb` for the current release.
- [ ] `brew tap edTheGuy00/tap && brew install fdemon && fdemon --version` succeeds on macOS (arm64) and verified on Linuxbrew (x86_64).
- [ ] A test release (or dry-run dispatch) confirms `bump-homebrew-tap` produces the expected formula change.
- [ ] README is updated.
- [ ] Issue #40 is closed.

### Phase 3 Complete When:
- [ ] AUR account `edTheGuy00` exists and `fdemon-bin` is published.
- [ ] `paru -S fdemon-bin` succeeds on a fresh Arch container; `fdemon --version` runs.
- [ ] A test release confirms `publish-aur` updates the PKGBUILD and `.SRCINFO` correctly.
- [ ] README is updated.

### Phase 4 Complete When:
- [ ] `DEVELOPMENT.md` documents both external channels, required secrets, and manual override procedures.
- [ ] README presents Homebrew → AUR → curl|bash ladder with one-liners for each.

---

## Rollout Order Recommendation

1. **Phase 1 first, standalone PR** — fixes the live bug for #41 quickly and doesn't depend on external accounts.
2. **Phase 2 second** — Homebrew is the largest user-base win and is what #40 specifically asks for. Bonus: setting up `HOMEBREW_TAP_TOKEN` proves out the "automated external-repo bump" pattern that Phase 3 reuses.
3. **Phase 3 third** — AUR has the smallest user pool but directly serves the original CachyOS reporter. Defer until after #40 is shipped to avoid context-switching the release pipeline twice.
4. **Phase 4 alongside Phase 3** — docs catch up once the channels are real.

---

## Open Questions for the User

Before producing the task breakdown (`TASKS.md`), please confirm:

1. **Tap repo name**: `edTheGuy00/homebrew-tap` (gives `brew install edTheGuy00/tap/fdemon`)? Or a different name like `homebrew-fdemon` (gives `brew install edTheGuy00/fdemon/fdemon`, less clean)?
2. **AUR account**: do you already have one as `edTheGuy00`, or do we need to register? If registering, that's a manual prerequisite step.
3. **Source variants**: do we want `fdemon` (source AUR) and `fdemon-git` (latest main) packages now, or defer until demand appears?
4. **`homebrew-core`**: any interest in submitting upstream, knowing it would require relicensing away from BSL-1.1?
5. **Release pipeline tarball contents**: OK to add `LICENSE` to the release tarballs (small `release.yml` change) so AUR can ship the license file cleanly?

---

## References

- Issue #41 (curl error / CachyOS): https://github.com/edTheGuy00/fdemon/issues/41
- Issue #40 (Homebrew request): https://github.com/edTheGuy00/fdemon/issues/40
- GitHub REST rate limits: https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api
- Homebrew formula cookbook: https://docs.brew.sh/Formula-Cookbook
- Homebrew tap docs: https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap
- Homebrew bump-formula action: https://github.com/dawidd6/action-homebrew-bump-formula
- Arch packaging standards: https://wiki.archlinux.org/title/Arch_package_guidelines
- AUR submission guidelines: https://wiki.archlinux.org/title/AUR_submission_guidelines
- AUR deploy action: https://github.com/KSXGitHub/github-actions-deploy-aur
- Current release workflow: `.github/workflows/release.yml`
- Current installer: `install.sh`
