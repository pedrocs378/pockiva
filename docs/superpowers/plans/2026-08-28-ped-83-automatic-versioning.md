# PED-83 Automatic Versioning and Release Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Pockiva version bumps consistent and make a `develop` to `main` release pull request recover from a missing bump through a protected, CI-validated GitHub App pull request.

**Architecture:** A tested Node.js versioning module owns all manifest changes, while a separate pure release-candidate module classifies explicit, missing, and invalid bumps. Task CI is restricted to pull requests targeting `develop`; a trusted `pull_request_target` workflow on `main` performs the minimal release check and, only for equality, uses a repository-scoped GitHub App to open an auto-merged patch pull request back to `develop`.

**Tech Stack:** Node.js 24.20.0 ESM and `node:test`, pnpm 11.24.0, GitHub Actions, GitHub CLI/API, GitHub App installation tokens, Tauri 2, Cargo stable, SemVer.

**Spec:** `docs/superpowers/specs/2026-08-28-ped-83-automatic-versioning-design.md`

## Global Constraints

- Work on parent branch `PED-83`, created from `develop`; child branches are `PED-84`, `PED-85`, and `PED-86`, created from the latest parent branch and squash-merged back in dependency order.
- Keep PED-84 -> PED-85 -> PED-86 -> PED-87 synchronized in Linear. Only the active child is `In Progress`; blocked children remain `Backlog`.
- Use one application version across `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/package.json`, `[workspace.package]` in `Cargo.toml`, and inherited local workspace entries in `Cargo.lock`.
- Apply one bump per delivery into `develop`. Child pull requests into `PED-83` do not bump independently; PED-84 records the parent delivery's bootstrap version `0.1.1` once.
- Use `patch` for fixes and small compatible changes, `minor` for compatible features, and `major` for incompatible behavior after `1.0.0`. In `0.x`, incompatible behavior advances `minor`, and `1.0.0` marks the first stable contract.
- Keep `develop` and `main` protected. The bot receives no bypass and never pushes directly to either branch.
- Pull requests targeting `develop` use squash merge. The release pull request from `develop` to `main` uses a regular merge commit.
- Pin every GitHub Action to an immutable 40-character commit and retain the current latest action-major policy.
- Keep updater signing material and GitHub App credentials out of logs, bundles, artifacts, and committed files.
- Run the full JavaScript and Rust verification suite before the parent pull request is merged.
- Native manual acceptance is macOS Apple Silicon. Windows x64 remains a required compile and release target but is not a manual acceptance blocker.

---

## File Map

- `scripts/version-bump.mjs`: pure SemVer calculation, transactional manifest rewriting, CLI, and reusable `--root` support.
- `scripts/version-bump.test.mjs`: isolated temporary-repository tests for patch/minor/major, rollback, malformed inputs, and the committed `0.1.1` state.
- `scripts/release-candidate.mjs`: pure comparison and base/candidate repository classification used by the release pull-request gate.
- `scripts/release-candidate.test.mjs`: explicit bump, equality fallback, decrease, and divergent metadata tests.
- `scripts/release-config.mjs`: shared stable-SemVer parser/comparator exports and root-aware release metadata validation.
- `scripts/release-config.test.mjs`: repository/workflow invariants, immutable action allow-list, minimal permissions, and trigger tests.
- `.github/workflows/ci.yml`: the three full task checks, triggered only by pull requests to `develop`.
- `.github/workflows/release-pr.yml`: one trusted `Validate release candidate` check and the guarded GitHub App patch-PR fallback.
- `.github/workflows/release.yml`: unchanged publication behavior, with tests confirming the final defense remains.
- `AGENTS.md`: SemVer selection, one-bump rule, fallback behavior, and automation-branch exception.
- `README.md`: contributor-facing bump command and release behavior.
- `docs/testing/release-pipeline.md`: operational setup, bootstrap sequence, recovery, and verification runbook.
- `package.json`: `version:bump` command and all release-related Node tests.
- `Cargo.toml`, `Cargo.lock`, `apps/desktop/package.json`, `apps/desktop/src-tauri/tauri.conf.json`: bootstrap version `0.1.1`.

---

### Task 1: PED-84 — Define SemVer policy and atomic bump command

**Files:**
- Create: `scripts/version-bump.mjs`
- Create: `scripts/version-bump.test.mjs`
- Modify: `scripts/release-config.mjs`
- Modify: `scripts/release-config.test.mjs`
- Modify: `package.json`
- Modify: `AGENTS.md`
- Modify: `README.md`
- Modify: `docs/testing/release-pipeline.md`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `apps/desktop/package.json`
- Modify: `apps/desktop/src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: `loadRepositoryReleaseMetadata(root)` and `validateReleaseMetadata(metadata)` from `scripts/release-config.mjs`.
- Produces: `parseStableVersion(value, label): number[]`, `compareStableVersions(left, right): -1 | 0 | 1`, `bumpStableVersion(version, kind): string`, and `applyVersionBump({ root, kind }): Promise<string>`.
- Internal helpers: `loadVersionFileTexts(root): Promise<Map<string, string>>`, `rewriteVersionFileTexts({ texts, currentVersion, nextVersion }): Map<string, string>`, `writeVersionFilesTransaction({ originalTexts, nextTexts }): Promise<void>`, and `restoreVersionFiles(originalTexts): Promise<void>`.
- Produces CLI: `pnpm version:bump patch|minor|major`, plus `node scripts/version-bump.mjs patch --root /tmp/pockiva-candidate --print-version` for a trusted workflow checkout.

- [ ] **Step 1: Start PED-84 from the parent branch and update Linear**

Run:

```bash
git switch PED-83
git switch -c PED-84
orca linear status set PED-84 --to "In Progress" --json
```

Expected: branch `PED-84` points at the design commit and Linear reports `In Progress`.

- [ ] **Step 2: Export stable SemVer parsing and comparison through failing tests**

Add to `scripts/release-config.test.mjs`:

```js
import { compareStableVersions, parseStableVersion } from './release-config.mjs'

it('parses and compares stable SemVer numerically', () => {
  assert.deepEqual(parseStableVersion('10.2.3', 'Version'), [10, 2, 3])
  assert.equal(compareStableVersions('0.1.10', '0.1.9'), 1)
  assert.equal(compareStableVersions('0.1.0', '0.1.0'), 0)
  assert.equal(compareStableVersions('0.0.9', '0.1.0'), -1)
  assert.throws(() => parseStableVersion('0.2.0-beta.1', 'Version'), /stable SemVer/)
})
```

Run: `node --test scripts/release-config.test.mjs`

Expected: FAIL because the two named exports do not exist.

- [ ] **Step 3: Implement the shared SemVer primitives**

In `scripts/release-config.mjs`, export the parser and add a comparator that compares all three numeric components in order:

```js
export const parseStableVersion = (version, label) => {
  const match = stableSemver.exec(version)
  if (!match) throw new Error(`${label} must be a stable SemVer value, received ${JSON.stringify(version)}`)
  return match.slice(1).map(Number)
}

export const compareStableVersions = (left, right) => {
  const leftParts = parseStableVersion(left, 'Left version')
  const rightParts = parseStableVersion(right, 'Right version')
  for (let index = 0; index < leftParts.length; index += 1) {
    if (leftParts[index] > rightParts[index]) return 1
    if (leftParts[index] < rightParts[index]) return -1
  }
  return 0
}
```

Refactor `assertVersionBump` to call `compareStableVersions(current, previous) <= 0` without changing its error text.

Run: `node --test scripts/release-config.test.mjs`

Expected: PASS.

- [ ] **Step 4: Write failing unit tests for the bump contract**

Create `scripts/version-bump.test.mjs` with a temporary fixture containing the four version-bearing files. Cover the pure calculation first:

```js
it('calculates patch, minor, and major versions', () => {
  assert.equal(bumpStableVersion('0.1.9', 'patch'), '0.1.10')
  assert.equal(bumpStableVersion('0.1.9', 'minor'), '0.2.0')
  assert.equal(bumpStableVersion('0.1.9', 'major'), '1.0.0')
  assert.throws(() => bumpStableVersion('0.1.9', 'feature'), /patch, minor, or major/)
})
```

Add fixture assertions proving `applyVersionBump` updates exactly `tauri.conf.json`, desktop `package.json`, the scoped workspace version in `Cargo.toml`, and only the `gameboy-desktop`, `gb-core`, and `gb-network` package blocks in `Cargo.lock`. Include a malformed lockfile test that leaves every original file byte-for-byte unchanged.

Run: `node --test scripts/version-bump.test.mjs`

Expected: FAIL because `scripts/version-bump.mjs` does not exist.

- [ ] **Step 5: Implement transactional version rewriting**

Create `scripts/version-bump.mjs` with these exports:

```js
const acceptedKinds = new Set(['patch', 'minor', 'major'])

export const bumpStableVersion = (version, kind) => {
  if (!acceptedKinds.has(kind)) throw new Error('Version kind must be patch, minor, or major')
  const [major, minor, patch] = parseStableVersion(version, 'Current version')
  if (kind === 'major') return `${major + 1}.0.0`
  if (kind === 'minor') return `${major}.${minor + 1}.0`
  return `${major}.${minor}.${patch + 1}`
}

export const applyVersionBump = async ({ root, kind }) => {
  const currentVersion = validateReleaseMetadata(await loadRepositoryReleaseMetadata(root))
  const nextVersion = bumpStableVersion(currentVersion, kind)
  const originalTexts = await loadVersionFileTexts(root)
  const nextTexts = rewriteVersionFileTexts({ texts: originalTexts, currentVersion, nextVersion })
  try {
    await writeVersionFilesTransaction({ originalTexts, nextTexts })
    const writtenVersion = validateReleaseMetadata(await loadRepositoryReleaseMetadata(root))
    if (writtenVersion !== nextVersion) throw new Error(`Expected written version ${nextVersion}, received ${writtenVersion}`)
  } catch (error) {
    await restoreVersionFiles(originalTexts)
    throw error
  }
  return nextVersion
}
```

Implementation requirements:

1. Call `validateReleaseMetadata(await loadRepositoryReleaseMetadata(root))` before calculating the next version.
2. Parse both JSON files and serialize with two spaces plus one trailing newline.
3. Replace exactly one version assignment inside `[workspace.package]` and verify its previous value equals the validated canonical version.
4. Locate exactly one Cargo lock package block for each of `gameboy-desktop`, `gb-core`, and `gb-network`; change no external `0.1.0` packages.
5. Calculate all output strings and validate their structure before writing any destination.
6. Write sibling temporary files, rename them into place, and restore the original strings if a later rename or post-write validation fails.
7. Reject an unknown argument, missing `--root` value, or extra positional argument before any write.

The CLI parser accepts exactly:

```text
node scripts/version-bump.mjs patch
node scripts/version-bump.mjs minor --root /absolute/repository
node scripts/version-bump.mjs major --root /absolute/repository --print-version
```

Run: `node --test scripts/version-bump.test.mjs scripts/release-config.test.mjs`

Expected: PASS, including rollback and external Cargo package preservation.

- [ ] **Step 6: Add the repository command and policy documentation**

Modify root `package.json`:

```json
"release:test": "node --test scripts/release-config.test.mjs scripts/version-bump.test.mjs",
"version:bump": "node scripts/version-bump.mjs"
```

Task 2 creates `scripts/release-candidate.test.mjs` and adds it to this command in the same commit.

Add the approved policy to `AGENTS.md` under the release rules. Explicitly state that the final parent PR owns a complex task's bump and its children do not bump. Document the literal commands `pnpm version:bump patch`, `pnpm version:bump minor`, and `pnpm version:bump major` in `README.md` and the release runbook.

Run:

```bash
pnpm lint
pnpm release:test
pnpm release:check
```

Expected: all commands PASS with the repository still at `0.1.0`.

- [ ] **Step 7: Apply and verify the PED-83 bootstrap patch version**

Run:

```bash
pnpm version:bump patch
pnpm release:check
cargo metadata --locked --no-deps --format-version 1
```

Expected: the command prints `0.1.1`; all published manifests and the three local Cargo lock packages contain `0.1.1`; external lock packages remain unchanged.

- [ ] **Step 8: Commit, push, review, and squash PED-84 into PED-83**

Run the focused tests again, then:

```bash
git add AGENTS.md README.md Cargo.toml Cargo.lock package.json apps/desktop/package.json apps/desktop/src-tauri/tauri.conf.json docs/testing/release-pipeline.md scripts/release-config.mjs scripts/release-config.test.mjs scripts/version-bump.mjs scripts/version-bump.test.mjs
git commit -m "feat(PED-84): add automatic SemVer bump command"
git push -u origin PED-84
gh pr create --base PED-83 --head PED-84 --title "PED-84: [Release] Definir política SemVer e comando de bump consistente" --body-file /tmp/ped-84-pr.md
```

The PR body records tests and the selected `patch` rationale. Attach the PR in Linear, move PED-84 to `In Review`, review the diff, then run `gh pr merge --squash --delete-branch`. Move PED-84 to `Done`, update local `PED-83` from `origin/PED-83`, and leave PED-85 blocked until the squash is visible.

---

### Task 2: PED-85 — Split task CI from release validation

**Files:**
- Create: `scripts/release-candidate.mjs`
- Create: `scripts/release-candidate.test.mjs`
- Create: `.github/workflows/release-pr.yml`
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/release-config.mjs`
- Modify: `scripts/release-config.test.mjs`
- Modify: `package.json`
- Modify: `docs/testing/release-pipeline.md`

**Interfaces:**
- Consumes: `compareStableVersions`, `loadRepositoryReleaseMetadata`, `validateReleaseMetadata`, and `bumpStableVersion`.
- Produces: `classifyReleaseCandidate({ baseVersion, candidateVersion }): { kind: 'valid', version: string, tag: string } | { kind: 'patch-required', version: string, tag: string, nextVersion: string, nextTag: string }`.
- Produces CLI: `node scripts/release-candidate.mjs --base-root /tmp/pockiva-main --candidate-root /tmp/pockiva-develop --print-json`.
- Produces required GitHub check: `Validate release candidate`.

- [ ] **Step 1: Start PED-85 only after PED-84 is integrated**

Run:

```bash
git switch PED-83
git pull --ff-only origin PED-83
git switch -c PED-85
orca linear status set PED-85 --to "In Progress" --json
```

Expected: PED-84 is `Done`, PED-85 is `In Progress`, and the branch contains version `0.1.1`.

- [ ] **Step 2: Write failing candidate-classification tests**

Create `scripts/release-candidate.test.mjs`:

```js
it('accepts explicit bumps and requests only a patch for equality', () => {
  assert.deepEqual(classifyReleaseCandidate({ baseVersion: '0.1.0', candidateVersion: '0.2.0' }), {
    kind: 'valid',
    version: '0.2.0',
    tag: 'v0.2.0'
  })
  assert.deepEqual(classifyReleaseCandidate({ baseVersion: '0.1.0', candidateVersion: '0.1.0' }), {
    kind: 'patch-required',
    version: '0.1.0',
    tag: 'v0.1.0',
    nextVersion: '0.1.1',
    nextTag: 'v0.1.1'
  })
  assert.throws(
    () => classifyReleaseCandidate({ baseVersion: '0.2.0', candidateVersion: '0.1.9' }),
    /must not be lower/
  )
})
```

Add temporary base/candidate root tests proving the CLI rejects divergent candidate metadata before it emits JSON.

Run: `node --test scripts/release-candidate.test.mjs`

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement the pure classifier and root-aware CLI**

Create `scripts/release-candidate.mjs`. `classifyReleaseCandidate` uses numeric comparison, not string comparison. The CLI loads both roots with `loadRepositoryReleaseMetadata`, validates both with `validateReleaseMetadata`, and prints exactly one JSON object to stdout when `--print-json` is supplied.

Run:

```bash
node --test scripts/release-candidate.test.mjs
node scripts/release-candidate.mjs --base-root . --candidate-root . --print-json
```

Expected: tests PASS; the repository-to-itself command prints `kind: patch-required` with `nextVersion: 0.1.2` because the committed branch is `0.1.1`.

- [ ] **Step 4: Restrict the full CI workflow through a failing structural test**

Extend `scripts/release-config.test.mjs` to require:

```js
assert.match(ciWorkflow, /pull_request:\s*\n\s*branches:\s*\[develop\]/)
assert.doesNotMatch(ciWorkflow, /^\s*push:/m)
assert.doesNotMatch(ciWorkflow, /branches:\s*\[main\]/)
```

Run: `node --test scripts/release-config.test.mjs`

Expected: FAIL because `.github/workflows/ci.yml` currently accepts every push and pull request.

- [ ] **Step 5: Limit CI to pull requests targeting develop**

Change only the event block in `.github/workflows/ci.yml`:

```yaml
on:
  pull_request:
    branches: [develop]
```

Keep all three job names and their implementation unchanged.

Run: `node --test scripts/release-config.test.mjs`

Expected: the new trigger assertions PASS and the existing action-pin assertions still PASS.

- [ ] **Step 6: Write a failing structural test for the minimal main gate**

Extend `scripts/release-config.test.mjs` to read `.github/workflows/release-pr.yml` and require:

- `pull_request_target`, base `main`, and types `opened`, `reopened`, `synchronize`;
- exactly one job named `Validate release candidate`;
- explicit guards for same-repository `develop`;
- top-level `contents: read` and `pull-requests: read`;
- `cancel-in-progress: true` keyed by pull-request number;
- trusted and candidate checkouts in separate directories;
- no `pnpm install`, Rust installation, platform build, release creation, or updater secrets.

Run: `node --test scripts/release-config.test.mjs`

Expected: FAIL because `.github/workflows/release-pr.yml` does not exist.

- [ ] **Step 7: Implement the read-only release gate**

Create `.github/workflows/release-pr.yml` with one job. Its early step fails unsupported sources explicitly rather than skipping the required job:

```yaml
- name: Validate release source
  env:
    HEAD_REF: ${{ github.event.pull_request.head.ref }}
    HEAD_REPOSITORY: ${{ github.event.pull_request.head.repo.full_name }}
  run: |
    test "$HEAD_REF" = "develop"
    test "$HEAD_REPOSITORY" = "$GITHUB_REPOSITORY"
```

Checkout `${{ github.event.pull_request.base.sha }}` into `trusted` with full history and `${{ github.event.pull_request.head.sha }}` into `candidate` with credentials disabled. Install Node from `trusted/.tool-versions`. Run the trusted classifier:

```bash
decision="$(node trusted/scripts/release-candidate.mjs \
  --base-root "$GITHUB_WORKSPACE/trusted" \
  --candidate-root "$GITHUB_WORKSPACE/candidate" \
  --print-json)"
```

For `valid`, read `.tag` from the JSON decision and query both `repos/$GITHUB_REPOSITORY/releases/tags/$decision_tag` and `repos/$GITHUB_REPOSITORY/git/ref/tags/$decision_tag` to fail on any collision. For `patch-required`, query `.nextTag` in the same two endpoints, then fail with a precise message that PED-86 will replace with the protected bot pull request. This intermediate gate is independently safe: it permits explicit bumps and never writes.

Run:

```bash
pnpm release:test
pnpm release:check
```

Expected: PASS; structural tests prove no build jobs exist in the main gate.

- [ ] **Step 8: Commit, push, review, and squash PED-85 into PED-83**

Update `package.json` so `release:test` lists all three Node test files. Update the runbook with the one-check main path and bootstrap caveat. Then:

```bash
git add .github/workflows/ci.yml .github/workflows/release-pr.yml package.json docs/testing/release-pipeline.md scripts/release-candidate.mjs scripts/release-candidate.test.mjs scripts/release-config.mjs scripts/release-config.test.mjs
git commit -m "ci(PED-85): split task and release validation"
git push -u origin PED-85
gh pr create --base PED-83 --head PED-85 --title "PED-85: [CI] Separar validações de tasks e de release" --body-file /tmp/ped-85-pr.md
```

Run the full local verification because child-branch PRs will no longer trigger `.github/workflows/ci.yml`. Attach the PR in Linear, move PED-85 to `In Review`, review it, squash-merge it, move PED-85 to `Done`, and update local `PED-83`.

---

### Task 3: PED-86 — Provision the GitHub App and protected patch pull request

**Files:**
- Modify: `.github/workflows/release-pr.yml`
- Modify: `scripts/release-config.test.mjs`
- Modify: `AGENTS.md`
- Modify: `README.md`
- Modify: `docs/testing/release-pipeline.md`

**External state:**
- Create GitHub App `pockiva-release-bot-pedrocs378`.
- Add repository secrets `POCKIVA_RELEASE_APP_ID` and `POCKIVA_RELEASE_APP_PRIVATE_KEY`.
- Enable repository auto-merge; keep automatic branch deletion enabled.
- Do not change branch-protection bypass settings.

**Interfaces:**
- Consumes: JSON `patch-required` decision with `nextVersion` and `nextTag` from Task 2.
- Produces deterministic branch `automation/release-pr-${releasePullRequestNumber}-patch` and one pull request targeting `develop`.
- Produces bot PR title `[Release] Bump Pockiva to v${nextVersion} for release PR #${releasePullRequestNumber}` and squash auto-merge request.

- [ ] **Step 1: Start PED-86 only after PED-85 is integrated**

Run:

```bash
git switch PED-83
git pull --ff-only origin PED-83
git switch -c PED-86
orca linear status set PED-86 --to "In Progress" --json
```

Expected: PED-85 is `Done` and PED-86 is `In Progress`.

- [ ] **Step 2: Resolve and pin the current GitHub App token action**

Use current official documentation, then resolve the latest release and immutable commit:

```bash
pockiva_token_tag="$(gh api repos/actions/create-github-app-token/releases/latest --jq '.tag_name')"
pockiva_token_object="$(gh api "repos/actions/create-github-app-token/git/ref/tags/$pockiva_token_tag" --jq '.object')"
pockiva_token_type="$(jq -r '.type' <<< "$pockiva_token_object")"
pockiva_token_sha="$(jq -r '.sha' <<< "$pockiva_token_object")"
if [ "$pockiva_token_type" = "tag" ]; then
  pockiva_token_sha="$(gh api "repos/actions/create-github-app-token/git/tags/$pockiva_token_sha" --jq '.object.sha')"
fi
printf '%s %s\n' "$pockiva_token_tag" "$pockiva_token_sha"
```

Use the printed 40-character commit SHA in the workflow. Record the printed tag in the workflow comment and add only the immutable action reference to the test allow-list.

- [ ] **Step 3: Write failing workflow tests for the automatic fallback**

Extend `scripts/release-config.test.mjs` to require all of these exact invariants:

```js
assert.match(releasePrWorkflow, /POCKIVA_RELEASE_APP_ID/)
assert.match(releasePrWorkflow, /POCKIVA_RELEASE_APP_PRIVATE_KEY/)
assert.match(releasePrWorkflow, /automation\/release-pr-\$\{\{ github\.event\.pull_request\.number \}\}-patch/)
assert.match(releasePrWorkflow, /gh pr create[\s\S]*--base develop/)
assert.match(releasePrWorkflow, /gh pr merge[\s\S]*--auto[\s\S]*--squash/)
assert.doesNotMatch(releasePrWorkflow, /push[^\n]*(develop|main)/)
assert.doesNotMatch(releasePrWorkflow, /force-with-lease|--force/)
```

Also require that App token creation and every mutating step are conditional on `patch-required`, while explicit bumps never reference the App secrets.

Run: `node --test scripts/release-config.test.mjs`

Expected: FAIL because the read-only workflow has no App path.

- [ ] **Step 4: Implement the idempotent App pull-request path**

Replace the equality failure with guarded steps that:

1. mint an installation token using the pinned `actions/create-github-app-token` action and the two secrets;
2. check for an existing open pull request whose head is the deterministic automation branch for the current release pull-request number and whose base is `develop`;
3. if it exists, confirm its version equals `nextVersion`, call `gh pr merge --auto --squash --delete-branch`, print its URL, and exit successfully;
4. if the remote branch exists without the expected open pull request, fail closed;
5. otherwise create the branch from the exact release pull-request head SHA, run the trusted script from `main` against the candidate checkout, and verify `git diff --name-only` equals this sorted set:

```text
Cargo.lock
Cargo.toml
apps/desktop/package.json
apps/desktop/src-tauri/tauri.conf.json
```

6. commit as `pockiva-release-bot[bot]` with the resolved message `chore(release): bump Pockiva to v${nextVersion}`;
7. push only `HEAD:refs/heads/${automationBranch}` with the App token;
8. create the bot pull request to `develop` and enable squash auto-merge.

Do not run candidate scripts after App credentials are available. The only executed versioning code comes from the trusted `main` checkout.

Run:

```bash
pnpm release:test
pnpm release:check
```

Expected: PASS, including immutable action and no-direct-push assertions.

- [ ] **Step 5: Document the generated-branch exception and recovery**

Add to `AGENTS.md`:

- the single pattern `automation/release-pr-${releasePullRequestNumber}-patch` is reserved for the GitHub App fallback and is not a feature branch;
- the bot PR still targets `develop`, runs the complete CI, and must use squash auto-merge;
- humans and agents may not use the automation namespace for ordinary work;
- explicit valid bumps are never rewritten.

Update `README.md` and `docs/testing/release-pipeline.md` with bot-PR lifecycle, deterministic reuse, failed-check recovery, and the two secret names without values.

- [ ] **Step 6: Commit and squash PED-86 into PED-83 before provisioning secrets**

Run:

```bash
git add .github/workflows/release-pr.yml AGENTS.md README.md docs/testing/release-pipeline.md scripts/release-config.test.mjs
git commit -m "ci(PED-86): automate protected patch bump PRs"
git push -u origin PED-86
gh pr create --base PED-83 --head PED-86 --title "PED-86: [Release] Configurar GitHub App para bump automático protegido" --body-file /tmp/ped-86-pr.md
```

Run the full local verification, attach the PR in Linear, move PED-86 to `In Review`, review it, squash-merge it, and update local `PED-83`. Keep PED-86 in `In Review` until external provisioning and secret-name verification complete.

- [ ] **Step 7: Create and install the least-privilege GitHub App**

In the authenticated GitHub settings UI, create `pockiva-release-bot-pedrocs378` with webhooks disabled, homepage `https://github.com/pedrocs378/pockiva`, repository permissions `Contents: Read and write` and `Pull requests: Read and write`, and no organization/account permissions. Install it only on `pedrocs378/pockiva`.

Generate one private key. Locate its exact download with `rg --files /Users/pedro/Downloads -g '*.private-key.pem'`, move that exact file to `/Users/pedro/.config/pockiva/release-app.pem`, set mode `0600`, and never print it. Store the numeric App ID through the hidden interactive prompt and the PEM through stdin:

```bash
gh secret set POCKIVA_RELEASE_APP_ID --repo pedrocs378/pockiva
gh secret set POCKIVA_RELEASE_APP_PRIVATE_KEY --repo pedrocs378/pockiva < /Users/pedro/.config/pockiva/release-app.pem
```

Paste the numeric App ID from the settings page only into the secret prompt. Do not copy it into logs, commits, or chat.

- [ ] **Step 8: Enable safe auto-merge and verify external state without exposing values**

Run:

```bash
gh api --method PATCH repos/pedrocs378/pockiva -f allow_auto_merge=true -f delete_branch_on_merge=true
gh api repos/pedrocs378/pockiva/installation --jq '{app_slug: .app_slug, repository_selection: .repository_selection}'
gh secret list --repo pedrocs378/pockiva
gh api repos/pedrocs378/pockiva/branches/develop/protection
gh api repos/pedrocs378/pockiva/branches/main/protection
```

Expected: the App slug matches the new App, both secret names exist, auto-merge and branch deletion are enabled, and neither protection response contains a bot bypass. Move PED-86 to `Done` only after these checks.

---

### Task 4: PED-87 — Integrate, publish, and validate Pockiva v0.1.1

**Files:**
- No repository files change in this task. If release evidence contradicts the committed runbook, stop PED-87 and create a concrete follow-up instead of editing a released branch ad hoc.

**External state:**
- Parent pull request `PED-83` to `develop`, squash merge.
- Bootstrap release pull request `develop` to `main`, regular merge commit.
- Published GitHub release and tag `v0.1.1`.
- Post-bootstrap `main` required context changed to `Validate release candidate`.

**Interfaces:**
- Consumes: integrated parent branch at version `0.1.1`, existing updater secrets, installed GitHub App, and all passing repository tests.
- Produces: public latest release `v0.1.1`, signed updater manifest for `darwin-aarch64` and `windows-x86_64`, and final branch-protection configuration.

- [ ] **Step 1: Mark PED-87 active and reconcile the parent branch**

Run:

```bash
git switch PED-83
git pull --ff-only origin PED-83
orca linear status set PED-87 --to "In Progress" --json
git status --short --branch
```

Expected: clean `PED-83`, version `0.1.1`, PED-84–86 `Done`, PED-87 `In Progress`.

- [ ] **Step 2: Run the complete local verification**

Run each command independently and preserve its exit status:

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm build
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Expected: every command exits `0`. Additionally run `pnpm release:check` and confirm it reports `v0.1.1` without a private-key marker in `apps/desktop/dist`.

- [ ] **Step 3: Push PED-83 and open the final parent pull request to develop**

Run:

```bash
git push -u origin PED-83
gh pr create --base develop --head PED-83 --title "PED-83: [Release] Automatizar versionamento e validação de releases" --body-file /tmp/ped-83-pr.md
```

The body lists PED-84–87, the `patch` rationale, the complete verification evidence, the GitHub App permissions, and the bootstrap exception. Attach the PR to PED-83 and PED-87 in Linear.

- [ ] **Step 4: Verify the three develop checks and squash-merge the parent**

Run:

```bash
pockiva_parent_pr="$(gh pr view PED-83 --json url --jq '.url')"
gh pr checks --watch "$pockiva_parent_pr"
gh pr merge "$pockiva_parent_pr" --squash --delete-branch
```

Expected: `Quality and tests`, `Desktop compile (macOS Apple Silicon)`, and `Desktop compile (Windows x64)` pass. The resulting `develop` commit is a squash commit, not a merge commit.

- [ ] **Step 5: Open the bootstrap release pull request**

Run:

```bash
gh pr create --base main --head develop --title "PED-83: [Release] Automatizar versionamento e validação de releases" --body-file /tmp/ped-83-release-pr.md
```

Expected: because the old `main` does not yet contain `.github/workflows/release-pr.yml`, this one bootstrap PR runs the three legacy checks. Confirm the diff contains `0.1.1` consistently and `main` still contains `0.1.0`.

- [ ] **Step 6: Merge the release pull request with a regular merge commit**

Run:

```bash
pockiva_release_pr="$(gh pr list --base main --head develop --state open --json url --jq '.[0].url')"
gh pr checks --watch "$pockiva_release_pr"
gh pr merge "$pockiva_release_pr" --merge
```

Expected: GitHub creates a two-parent merge commit on `main`; the post-merge `Release` workflow starts exactly once for that commit.

- [ ] **Step 7: Watch the release workflow and validate the published assets**

Resolve and watch the newest release run, then verify its head SHA equals the release merge commit before accepting it:

```bash
pockiva_release_run_id="$(gh run list --workflow Release --branch main --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run view "$pockiva_release_run_id" --json headSha,event,url
gh run watch "$pockiva_release_run_id" --exit-status
gh release view v0.1.1 --repo pedrocs378/pockiva --json tagName,isDraft,isPrerelease,targetCommitish,assets,url
gh api repos/pedrocs378/pockiva/releases/latest --jq '.tag_name'
gh release download v0.1.1 --repo pedrocs378/pockiva --pattern latest.json --dir /tmp/pockiva-v0.1.1
jq -e '.version == "0.1.1" and (.platforms | has("darwin-aarch64") and has("windows-x86_64"))' /tmp/pockiva-v0.1.1/latest.json
jq -e '.platforms."darwin-aarch64".signature | length > 0' /tmp/pockiva-v0.1.1/latest.json
jq -e '.platforms."windows-x86_64".signature | length > 0' /tmp/pockiva-v0.1.1/latest.json
```

Expected: release is public, latest, not draft/prerelease, points to the release merge commit, and includes macOS Apple Silicon, Windows x64, signatures, and `latest.json`.

- [ ] **Step 8: Switch main protection to the minimal release check**

After `main` contains `.github/workflows/release-pr.yml`, update only `main.required_status_checks` to strict mode with the single context `Validate release candidate`. Preserve zero required approvals, administrator enforcement, conversation resolution, deletion protection, and force-push protection. Leave the three existing contexts on `develop`.

Run:

```bash
gh api --method PATCH repos/pedrocs378/pockiva/branches/main/protection/required_status_checks \
  -F strict=true \
  -f 'contexts[]=Validate release candidate'
```

Read both protection documents after the update and assert:

```text
main:    Validate release candidate
develop: Quality and tests
         Desktop compile (macOS Apple Silicon)
         Desktop compile (Windows x64)
```

- [ ] **Step 9: Verify future-flow readiness without publishing another release**

Run `gh workflow view release-pr.yml --ref main`, `gh secret list --repo pedrocs378/pockiva`, and `gh api repos/pedrocs378/pockiva/installation --jq '.app_slug'`. Do not open a second `develop` to `main` pull request and do not bump to `0.1.2` merely to test the fallback. The Node tests and structural workflow tests are the acceptance evidence for equality; the first real missing-bump release will exercise the App PR end to end.

- [ ] **Step 10: Close Linear work with evidence**

Attach the parent and release pull requests to PED-83/PED-87. Add one completion comment with the release URL, merge SHA, workflow run URL, and verification summary. Move PED-87 to `Done`; move PED-83 to `Done` only after PED-84–87 are all `Done`. Keep PED-32 `In Progress` because PED-40 remains independently `Ready for dev`.

---

## Final Acceptance Checklist

- [ ] `AGENTS.md` contains the compatible SemVer policy, one-bump rule, and automation exception.
- [ ] `pnpm version:bump patch|minor|major` changes only the expected version files and rolls back on failure.
- [ ] All shipped metadata and local workspace lock entries declare `0.1.1`.
- [ ] Task pull requests to `develop` execute exactly the three full checks.
- [ ] Future release pull requests to `main` execute only `Validate release candidate`.
- [ ] Explicit greater versions pass without App credentials or writes.
- [ ] Equal versions plan exactly one next-patch bot pull request into `develop`.
- [ ] The bot pull request has no bypass, runs full CI, and uses squash auto-merge.
- [ ] `develop` to `main` remains a human-controlled regular merge commit.
- [ ] `v0.1.1` is published only after both native artifacts and signed updater entries succeed.
- [ ] No GitHub App private key, updater private key, or secret value appears in committed files, logs, bundles, or final reporting.
