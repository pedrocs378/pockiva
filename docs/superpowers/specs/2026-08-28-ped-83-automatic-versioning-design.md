# PED-83 — Automatic versioning and release validation design

## Context

The first `develop` to `main` pull request failed because all published manifests still declared `0.1.0`, which is also the version already present on `main`. The release guard behaved correctly when it rejected an unchanged version. The missing pieces are a repeatable version-selection policy, one atomic bump command, a lightweight release pull-request gate, and a safe fallback when a release pull request reaches `main` without an explicit bump.

PED-83 is a child orchestration issue of PED-32. Its dependency chain is:

```text
PED-84 Policy and bump command
   -> PED-85 Task CI and release validation split
      -> PED-86 GitHub App and protected automatic bump
         -> PED-87 Publish and validate v0.1.1
```

## Decisions

### Version ownership

`apps/desktop/src-tauri/tauri.conf.json` remains the canonical application version. The same version must be committed in:

- `apps/desktop/package.json`;
- `Cargo.toml` under `[workspace.package]`;
- local workspace package entries in `Cargo.lock` that inherit the workspace version.

One command will update these files together and validate the result before it exits successfully. Partial updates are errors.

### SemVer policy

A version changes once per delivery into `develop`, not once per commit or sub-issue.

- A simple task chooses its bump before its pull request is merged into `develop`.
- A complex task chooses one bump in the final parent pull request. Its sub-issue pull requests into the parent branch do not bump versions independently.
- `patch` is used for bug fixes, documentation, CI changes, internal refactors, and small backward-compatible adjustments.
- `minor` is used for new backward-compatible user-facing capabilities.
- `major` is used for incompatible public behavior after `1.0.0`.
- While Pockiva remains in `0.x`, incompatible changes increase `minor`; `1.0.0` marks the first stable public contract.

Change size alone does not determine the bump. Compatibility and user-visible behavior do.

If a `develop` to `main` pull request has no explicit bump, the release automation chooses `patch`. It never replaces an explicit valid `minor` or `major` bump.

### Protected automatic fallback

The fallback will not push directly to `develop` and will not receive a branch-protection bypass. A dedicated GitHub App will create a short-lived branch and pull request targeting `develop`. That pull request must pass the same quality, macOS, and Windows checks as every other task and will be integrated by squash merge.

The generated branch uses `automation/release-pr-<number>-patch`. This is the only documented exception to the Linear-ID branch naming rule because it is generated release housekeeping rather than a new implementation task. The generated pull-request title identifies the source release pull request and the resulting version.

The GitHub App is installed only on `pedrocs378/pockiva` and receives:

- Contents: read and write;
- Pull requests: read and write;
- implicit metadata read access.

It receives no administration permission and no bypass on `develop` or `main`. Its installation token is short-lived. The App ID and private key are stored as the `POCKIVA_RELEASE_APP_ID` and `POCKIVA_RELEASE_APP_PRIVATE_KEY` GitHub secrets and never exposed to frontend builds, shell traces, artifacts, or pull-request content.

## Components

### Version bump command

`scripts/version-bump.mjs` will expose a repository command with exactly three accepted bump kinds: `patch`, `minor`, and `major`. It will:

1. load and validate the current stable SemVer from all published manifests;
2. calculate the requested next version;
3. update the Tauri configuration, desktop package, Cargo workspace, and inherited local package entries in `Cargo.lock`;
4. reload all files and run the existing release metadata validation;
5. print the resulting version for local use and workflow outputs.

The command supports an explicit repository root so a trusted copy of the script can modify a separate checkout. It rejects prerelease/build metadata, unknown bump kinds, divergent starting versions, malformed manifests, and unexpected lockfile structure.

### Task CI

`.github/workflows/ci.yml` will run only for pull requests whose base branch is `develop`. It will retain the existing required jobs:

- `Quality and tests`;
- `Desktop compile (macOS Apple Silicon)`;
- `Desktop compile (Windows x64)`.

The workflow will no longer run for pushes or pull requests whose base is `main`.

### Release pull-request gate

A separate workflow will run for pull requests whose base branch is `main`. It will expose one required check named `Validate release candidate` and fail explicitly unless all of the following are true:

- the head branch is `develop`;
- the head repository is `pedrocs378/pockiva`;
- the three published versions are internally consistent stable SemVer values;
- the candidate version is equal to or greater than the version on the pull-request base.

When the candidate is greater than the base version, its tag must not already be published and the check succeeds without write permissions or bot activity. When the candidate is equal, the expected patch tag must not already exist before recovery starts. A lower, inconsistent, invalid, or colliding version fails without attempting recovery. Equality is the only condition that triggers the automatic patch fallback.

### Automatic patch pull request

For an equal version, the privileged portion of the release gate will:

1. authenticate as the dedicated GitHub App;
2. resolve the expected patch version from the current `main` version;
3. reuse or create the deterministic automation branch from the current `develop` head;
4. run the trusted bump command against that checkout;
5. commit only the expected version files and push the automation branch;
6. reuse or create one pull request targeting `develop`;
7. enable squash auto-merge for that pull request.

The App token, rather than the default `GITHUB_TOKEN`, creates the branch and pull request. This ensures the resulting pull-request events can run the normal CI without GitHub's recursive-workflow suppression or manual approval state.

Repository auto-merge is enabled as part of provisioning. It remains constrained by the existing required checks and branch protection; enabling it does not allow the App to merge a failing pull request.

The first release-gate run may report that the patch pull request is pending. After its checks pass and GitHub merges it, `develop` advances, the original release pull request updates automatically, and the release gate runs again. The second run sees a greater version and succeeds.

Idempotency is based on the source release pull-request number. Reopened, synchronized, or manually rerun workflows reuse the same branch and pull request. If the branch contains unexpected changes, the workflow fails instead of overwriting it. The workflow never force-pushes.

### Release workflow

The existing post-merge release workflow remains responsible for creating the draft, building sequential macOS Apple Silicon and Windows x64 artifacts, aggregating `latest.json`, and publishing only after both platforms succeed. It continues to reject equal/decreased versions as a final defense.

The first release is a bootstrap exception. The release-gate workflow does not yet exist on the current `main`, so PED-84 commits the explicit patch version `0.1.1`. The first `develop` to `main` pull request therefore uses the checks already present on `main` and publishes `v0.1.1`. After that merge, the new release gate exists on `main`; branch protection is then changed to require only `Validate release candidate` for future release pull requests.

## Security model

The privileged workflow uses `pull_request_target` so its definition comes from the trusted `main` base branch. It verifies the exact same-repository `develop` source before requesting App credentials. Code from the release pull request is not executed with the App private key. The trusted bump script is loaded from `main` and receives a separate `develop` checkout as data. Every third-party action, including token generation, is pinned to an immutable commit.

Repository workflow permissions remain read-only by default. Only the App installation token receives write access, and only within the guarded automatic-bump steps. The bot pull request follows normal branch protection, required checks, conversation resolution, and squash merge rules.

Forks, arbitrary head branches, malformed versions, divergent manifests, stale automation branches, and unexpected file changes terminate without writes. Logs may include version numbers, branch names, pull-request numbers, and commit SHAs, but never secret material.

## Failure and recovery

- Network or GitHub API failure leaves the release pull request open and retryable.
- A failed bot pull-request check leaves that pull request open for diagnosis; the release pull request does not pass.
- A previously created automation branch or pull request is reused after a rerun.
- An explicit bump that is not greater than `main` fails; only exact equality receives patch recovery.
- A published tag collision fails and never increments again automatically, preventing an accidental second release.
- A failed post-merge platform build preserves the draft release under the existing recovery behavior.

No workflow automatically merges `develop` into `main`. The human-controlled release boundary remains the regular merge commit of the release pull request.

## Repository governance

`AGENTS.md` will record the SemVer policy, the single-bump rule for parent deliveries, the automatic patch fallback, and the generated-branch exception. Existing merge rules remain unchanged:

- feature, task, sub-issue, and parent pull requests targeting `develop` use squash merge;
- release pull requests from `develop` to `main` use a regular merge commit;
- direct pushes to `develop` and `main` remain prohibited.

`develop` keeps its three required CI contexts. After the bootstrap release, `main` replaces those contexts with `Validate release candidate`, with strict branch updating, administrator enforcement, conversation resolution, and deletion/force-push protection preserved.

## Verification

Automated tests cover:

- patch, minor, and major calculations, including the `0.x` policy;
- atomic updates of every published manifest and inherited Cargo lock entry;
- invalid arguments, unstable SemVer, starting-version divergence, and malformed files;
- explicit release bumps, equality fallback, decreased versions, and published-tag collisions;
- workflow triggers, same-repository/head guards, immutable action revisions, minimal permissions, idempotent automation naming, and absence of direct protected-branch pushes;
- task CI restricted to `develop` and the single release gate restricted to `main`;
- absence of signing keys and GitHub App private-key material from web bundles.

Repository verification runs the full existing JavaScript and Rust suite before review. GitHub verification confirms the App installation and secret names without revealing values, the bot pull-request behavior, branch protection contexts, merge strategies, the published `v0.1.1` tag, both native installers, updater signatures, and `latest.json`.

Windows x64 must continue to compile and ship through GitHub Actions. Native acceptance remains macOS Apple Silicon only, consistent with PED-32.
