# Release pipeline validation

This runbook validates Pockiva's updater and release path without publishing a release. Task pull requests targeting `develop` run the complete three-check CI, while release pull requests targeting `main` run only the read-only `Validate release candidate` check. The privileged publishing workflow runs only after a pull request from `develop` is merged into `main`; feature and orchestration pull requests cannot create tags or releases.

## Pull-request checks

- Pull requests targeting `develop` run `Quality and tests`, `Desktop compile (macOS Apple Silicon)`, and `Desktop compile (Windows x64)`. The full CI does not run on pushes or on pull requests targeting another branch.
- Pull requests targeting `main` must come from `develop` in `pedrocs378/pockiva`. Their only required workflow check is `Validate release candidate`, which compares metadata from isolated base and candidate checkouts using the trusted script from `main`.
- A candidate version greater than `main` passes only when its release and Git tag do not exist. A lower, malformed, divergent, or colliding version fails closed.
- Until PED-86 adds the protected GitHub App recovery, a candidate equal to `main` verifies that the expected patch tag is available and then fails with an explicit PED-86 message. This gate is read-only: it does not create branches, pull requests, tags, releases, or builds.

The first `develop -> main` release is a bootstrap exception. GitHub loads `pull_request_target` workflows from the base branch, and the current `main` does not contain `release-pr.yml` yet. The first release pull request therefore carries the explicit `0.1.1` bump from PED-84 and uses the checks already configured on `main`. After that merge and the PED-86 provisioning, require only `Validate release candidate` on `main`; keep the three task CI contexts required on `develop`.

## Pre-release checks

1. Confirm the release pull request is exactly `develop -> main` in `pedrocs378/pockiva`.
2. Choose one bump for the delivery into `develop`: `patch` for fixes, documentation, CI, refactors, and small compatible changes; `minor` for compatible user-facing capabilities; or `major` for incompatible behavior after `1.0.0`. In `0.x`, incompatible behavior advances `minor`. For a complex task, only its final parent pull request bumps.
3. Run exactly one atomic command: `pnpm version:bump patch`, `pnpm version:bump minor`, or `pnpm version:bump major`. It updates the canonical Tauri version, desktop package, Cargo workspace version, and the inherited `gameboy-desktop`, `gb-core`, and `gb-network` entries in `Cargo.lock` together.
4. Review the four changed version files and run `pnpm release:check`. The check rejects mismatched or invalid versions, invalid updater configuration, missing capabilities, and an unsafe release pull-request gate. The `Validate release candidate` workflow performs the comparison against `main` and checks release/tag collisions.
5. Run the complete repository verification documented in `AGENTS.md`.
6. Confirm the GitHub `release` environment is restricted to `main` and contains both updater signing secret names. Never print their values.
7. Merge the release pull request with a merge commit. Do not squash `develop -> main`, because the merged commit is the immutable release source.

## Expected artifacts

The workflow creates or resumes an unpublished draft for `v<version>`, then runs the platforms sequentially:

1. macOS Apple Silicon (`aarch64-apple-darwin`): app bundle, DMG, updater archive, and signature.
2. Windows x64 (`x86_64-pc-windows-msvc`): NSIS installer, updater package, and signature.
3. Aggregated `latest.json` containing signed `darwin-aarch64` and `windows-x86_64` entries.

Only after both platform entries and signatures are present does the workflow publish the draft and mark it as the latest release. The frontend bundle is scanned after each native build to ensure that updater private-key material was not exposed by Vite.

## Failure and recovery

If preparation or either build fails, keep the GitHub release as a draft and inspect the failed job. After fixing the cause, rerun the failed workflow for the same merged commit. Preparation accepts the existing draft only when its tag still points to that exact commit; a published version, a conflicting tag, or a draft pointing elsewhere fails closed.

Never delete and recreate the updater signing key after users have installed a signed release. Restore the encrypted key from `~/.config/pockiva/updater.key` and its password from the macOS Keychain service `com.pedro.pockiva.updater`, or plan an explicit signing-key migration.

## Validation without a release

For changes that must not publish a release:

- open pull requests only into the orchestration branch or `develop`, never `main`;
- for pull requests targeting `develop`, verify the quality, macOS Apple Silicon, and Windows x64 checks;
- for child pull requests targeting an orchestration branch, run the complete local verification from `AGENTS.md`, because the task CI is intentionally restricted to `develop`;
- inspect repository visibility, default branch, branch protection, environment configuration, and secret names through GitHub;
- confirm `main` still points to the preserved baseline and that no `v0.1.0` tag or release exists.

Apple notarization and Windows Authenticode are not part of the current pipeline, so downloaded installers can still trigger Gatekeeper or SmartScreen warnings.
