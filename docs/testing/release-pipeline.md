# Release pipeline validation

This runbook validates Pockiva's updater and release path without publishing a release. The privileged workflow runs only after a pull request from `develop` is merged into `main`; feature and orchestration pull requests cannot create tags or releases.

## Pre-release checks

1. Confirm the release pull request is exactly `develop -> main` in `pedrocs378/pockiva`.
2. Choose one bump for the delivery into `develop`: `patch` for fixes, documentation, CI, refactors, and small compatible changes; `minor` for compatible user-facing capabilities; or `major` for incompatible behavior after `1.0.0`. In `0.x`, incompatible behavior advances `minor`. For a complex task, only its final parent pull request bumps.
3. Run exactly one atomic command: `pnpm version:bump patch`, `pnpm version:bump minor`, or `pnpm version:bump major`. It updates the canonical Tauri version, desktop package, Cargo workspace version, and the inherited `gameboy-desktop`, `gb-core`, and `gb-network` entries in `Cargo.lock` together.
4. Review the four changed version files and run `pnpm release:check`. The check rejects mismatched or non-increasing versions, invalid updater configuration, missing capabilities, and mutable action references.
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
- verify quality, macOS Apple Silicon, and Windows x64 checks on the pull request;
- inspect repository visibility, default branch, branch protection, environment configuration, and secret names through GitHub;
- confirm `main` still points to the preserved baseline and that no `v0.1.0` tag or release exists.

Apple notarization and Windows Authenticode are not part of the current pipeline, so downloaded installers can still trigger Gatekeeper or SmartScreen warnings.
