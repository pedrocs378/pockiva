import assert from 'node:assert/strict'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { after, describe, it } from 'node:test'
import {
  assertBundleHasNoSigningKey,
  assertVersionBump,
  compareStableVersions,
  loadRepositoryReleaseMetadata,
  parseStableVersion,
  validateReleaseMetadata,
  validateReleaseWorkflow
} from './release-config.mjs'

const validMetadata = () => ({
  tauri: {
    productName: 'Pockiva',
    version: '0.1.0',
    identifier: 'com.pedro.pockiva',
    bundle: { createUpdaterArtifacts: true },
    plugins: {
      updater: {
        pubkey: 'public-key',
        endpoints: ['https://github.com/pedrocs378/pockiva/releases/latest/download/latest.json']
      }
    }
  },
  desktopPackage: { version: '0.1.0' },
  cargoVersion: '0.1.0',
  capabilities: { permissions: ['process:default', 'updater:default'] },
  viteConfig: "envPrefix: ['VITE_']"
})

describe('release metadata', () => {
  it('parses and compares stable SemVer numerically', () => {
    assert.deepEqual(parseStableVersion('10.2.3', 'Version'), [10, 2, 3])
    assert.equal(compareStableVersions('0.1.10', '0.1.9'), 1)
    assert.equal(compareStableVersions('0.1.0', '0.1.0'), 0)
    assert.equal(compareStableVersions('0.0.9', '0.1.0'), -1)
    assert.throws(() => parseStableVersion('0.2.0-beta.1', 'Version'), /stable SemVer/)
  })

  it('accepts one stable version and the signed public updater contract', () => {
    assert.equal(validateReleaseMetadata(validMetadata()), '0.1.0')
  })

  it('rejects prerelease versions and mismatched shipped metadata', () => {
    const prerelease = validMetadata()
    prerelease.tauri.version = '0.2.0-beta.1'
    assert.throws(() => validateReleaseMetadata(prerelease), /stable SemVer/)

    const mismatch = validMetadata()
    mismatch.desktopPackage.version = '0.2.0'
    assert.throws(() => validateReleaseMetadata(mismatch), /must match/)
  })

  it('requires every release pull request to increase the version', () => {
    assert.doesNotThrow(() => assertVersionBump('0.2.0', '0.1.9'))
    assert.throws(() => assertVersionBump('0.1.0', '0.1.0'), /must be greater/)
    assert.throws(() => assertVersionBump('0.1.9', '0.2.0'), /must be greater/)
  })

  it('validates the committed repository metadata', async () => {
    const metadata = await loadRepositoryReleaseMetadata()
    assert.equal(validateReleaseMetadata(metadata), metadata.tauri.version)
  })
})

describe('signing key isolation', () => {
  const directories = []
  after(async () => Promise.all(directories.map((directory) => rm(directory, { recursive: true, force: true }))))

  it('rejects a generic private-key marker in a built bundle', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'pockiva-release-'))
    directories.push(directory)
    await writeFile(join(directory, 'index.js'), 'untrusted comment: minisign encrypted secret key')

    await assert.rejects(() => assertBundleHasNoSigningKey(directory), /private signing material/)
  })

  it('rejects the exact configured signing key when supplied by CI', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'pockiva-release-'))
    directories.push(directory)
    await writeFile(join(directory, 'index.js'), 'prefix super-secret-private-key suffix')

    await assert.rejects(
      () => assertBundleHasNoSigningKey(directory, 'super-secret-private-key'),
      /private signing material/
    )
  })
})

describe('release workflow', () => {
  it('requires the guarded develop-to-main release path and draft aggregation', async () => {
    await assert.doesNotReject(() => validateReleaseWorkflow())
  })

  it('accepts workflows checked out with Windows CRLF line endings', async () => {
    const root = await mkdtemp(join(tmpdir(), 'pockiva-release-workflow-'))
    try {
      const workflowDirectory = join(root, '.github/workflows')
      await mkdir(workflowDirectory, { recursive: true })
      const [releaseWorkflow, releasePrWorkflow] = await Promise.all([
        readFile(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8'),
        readFile(new URL('../.github/workflows/release-pr.yml', import.meta.url), 'utf8')
      ])
      await Promise.all([
        writeFile(join(workflowDirectory, 'release.yml'), releaseWorkflow.replaceAll('\n', '\r\n')),
        writeFile(join(workflowDirectory, 'release-pr.yml'), releasePrWorkflow.replaceAll('\n', '\r\n'))
      ])

      await assert.doesNotReject(() => validateReleaseWorkflow(root))
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  it('resumes draft releases and publishes assets through the release id', async () => {
    const workflow = await readFile(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8')

    assert.match(workflow, /repos\/\$GITHUB_REPOSITORY\/releases\?per_page=100/)
    assert.doesNotMatch(workflow, /repos\/\$GITHUB_REPOSITORY\/releases\/tags\/\$TAG/)
    assert.match(workflow, /--method POST "repos\/\$GITHUB_REPOSITORY\/releases"/)
    assert.match(workflow, /repos\/\$GITHUB_REPOSITORY\/releases\/\$RELEASE_ID\/assets\?per_page=100/)
    assert.match(workflow, /repos\/\$GITHUB_REPOSITORY\/releases\/assets\/\$manifest_asset_id/)
    assert.match(workflow, /--method POST "repos\/\$GITHUB_REPOSITORY\/git\/refs"/)
    assert.match(workflow, /--method PATCH "repos\/\$GITHUB_REPOSITORY\/releases\/\$RELEASE_ID"/)
    assert.doesNotMatch(workflow, /gh release create/)
    assert.doesNotMatch(workflow, /gh release download/)
    assert.doesNotMatch(workflow, /gh release edit/)
  })

  it('rejects write permissions in the release pull-request gate', async () => {
    const root = await mkdtemp(join(tmpdir(), 'pockiva-release-workflow-'))
    try {
      const workflowDirectory = join(root, '.github/workflows')
      await mkdir(workflowDirectory, { recursive: true })
      const [releaseWorkflow, releasePrWorkflow] = await Promise.all([
        readFile(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8'),
        readFile(new URL('../.github/workflows/release-pr.yml', import.meta.url), 'utf8')
      ])
      await Promise.all([
        writeFile(join(workflowDirectory, 'release.yml'), releaseWorkflow),
        writeFile(
          join(workflowDirectory, 'release-pr.yml'),
          releasePrWorkflow.replace('contents: read', 'contents: write')
        )
      ])

      await assert.rejects(() => validateReleaseWorkflow(root), /read-only permissions/)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  it('rejects job-level GITHUB_TOKEN permissions even when top-level permissions stay read-only', async () => {
    const root = await mkdtemp(join(tmpdir(), 'pockiva-release-workflow-'))
    try {
      const workflowDirectory = join(root, '.github/workflows')
      await mkdir(workflowDirectory, { recursive: true })
      const [releaseWorkflow, releasePrWorkflow] = await Promise.all([
        readFile(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8'),
        readFile(new URL('../.github/workflows/release-pr.yml', import.meta.url), 'utf8')
      ])
      const workflowWithJobWrite = releasePrWorkflow.replace(
        '    runs-on: ubuntu-24.04',
        '    runs-on: ubuntu-24.04\n    permissions:\n      contents: write'
      )
      await Promise.all([
        writeFile(join(workflowDirectory, 'release.yml'), releaseWorkflow),
        writeFile(join(workflowDirectory, 'release-pr.yml'), workflowWithJobWrite)
      ])

      await assert.rejects(() => validateReleaseWorkflow(root), /exactly one top-level read-only permissions block/)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  it('rejects job-level GITHUB_TOKEN permissions using the write-all scalar form', async () => {
    const root = await mkdtemp(join(tmpdir(), 'pockiva-release-workflow-'))
    try {
      const workflowDirectory = join(root, '.github/workflows')
      await mkdir(workflowDirectory, { recursive: true })
      const [releaseWorkflow, releasePrWorkflow] = await Promise.all([
        readFile(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8'),
        readFile(new URL('../.github/workflows/release-pr.yml', import.meta.url), 'utf8')
      ])
      const workflowWithJobWriteAll = releasePrWorkflow.replace(
        '    runs-on: ubuntu-24.04',
        '    runs-on: ubuntu-24.04\n    permissions: write-all'
      )
      await Promise.all([
        writeFile(join(workflowDirectory, 'release.yml'), releaseWorkflow),
        writeFile(join(workflowDirectory, 'release-pr.yml'), workflowWithJobWriteAll)
      ])

      await assert.rejects(() => validateReleaseWorkflow(root), /exactly one top-level read-only permissions block/)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  it('rejects job-level GITHUB_TOKEN permissions using the inline mapping form', async () => {
    const root = await mkdtemp(join(tmpdir(), 'pockiva-release-workflow-'))
    try {
      const workflowDirectory = join(root, '.github/workflows')
      await mkdir(workflowDirectory, { recursive: true })
      const [releaseWorkflow, releasePrWorkflow] = await Promise.all([
        readFile(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8'),
        readFile(new URL('../.github/workflows/release-pr.yml', import.meta.url), 'utf8')
      ])
      const workflowWithInlineJobWrite = releasePrWorkflow.replace(
        '    runs-on: ubuntu-24.04',
        '    runs-on: ubuntu-24.04\n    permissions: { contents: write }'
      )
      await Promise.all([
        writeFile(join(workflowDirectory, 'release.yml'), releaseWorkflow),
        writeFile(join(workflowDirectory, 'release-pr.yml'), workflowWithInlineJobWrite)
      ])

      await assert.rejects(() => validateReleaseWorkflow(root), /exactly one top-level read-only permissions block/)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  it('runs the full task CI only for pull requests targeting develop', async () => {
    const ciWorkflow = await readFile(new URL('../.github/workflows/ci.yml', import.meta.url), 'utf8')

    assert.match(ciWorkflow, /pull_request:\s*\n\s*branches:\s*\[develop\]/)
    assert.doesNotMatch(ciWorkflow, /^\s*push:/m)
    assert.doesNotMatch(ciWorkflow, /branches:\s*\[main\]/)
  })

  it('defines one top-level read-only release candidate check for pull requests targeting main', async () => {
    const workflow = await readFile(new URL('../.github/workflows/release-pr.yml', import.meta.url), 'utf8')

    assert.match(
      workflow,
      /pull_request_target:\s*\n\s*branches:\s*\[main\]\s*\n\s*types:\s*\[opened, reopened, synchronize\]/
    )
    assert.equal(workflow.match(/^\s{4}name:\s*Validate release candidate\s*$/gm)?.length, 1)
    assert.equal(workflow.match(/^\s{4}runs-on:/gm)?.length, 1)
    assert.equal(workflow.match(/^\s*permissions:\s*$/gm)?.length, 1)
    assert.doesNotMatch(workflow, /^[ \t]+permissions:\s*$/m)
    assert.match(workflow, /^permissions:\n {2}contents: read\n {2}pull-requests: read$/m)
    assert.doesNotMatch(workflow, /^ {2}\S[^#\n]*:\s*write\s*$/m)
    assert.match(workflow, /group:\s*release-pr-\$\{\{ github\.event\.pull_request\.number \}\}/)
    assert.match(workflow, /cancel-in-progress:\s*true/)

    assert.match(workflow, /HEAD_REF:\s*\$\{\{ github\.event\.pull_request\.head\.ref \}\}/)
    assert.match(workflow, /HEAD_REPOSITORY:\s*\$\{\{ github\.event\.pull_request\.head\.repo\.full_name \}\}/)
    assert.match(workflow, /\[\[ "\$HEAD_REF" != "develop" \]\]/)
    assert.match(workflow, /\[\[ "\$HEAD_REPOSITORY" != "\$GITHUB_REPOSITORY" \]\]/)

    assert.match(workflow, /ref:\s*\$\{\{ github\.workflow_sha \}\}[\s\S]*path:\s*trusted[\s\S]*fetch-depth:\s*0/)
    assert.match(
      workflow,
      /ref:\s*\$\{\{ github\.event\.pull_request\.base\.sha \}\}[\s\S]*path:\s*base[\s\S]*persist-credentials:\s*false/
    )
    assert.match(
      workflow,
      /ref:\s*\$\{\{ github\.event\.pull_request\.head\.sha \}\}[\s\S]*path:\s*candidate[\s\S]*persist-credentials:\s*false/
    )
    assert.match(workflow, /node-version-file:\s*trusted\/\.tool-versions/)
    assert.match(workflow, /node trusted\/scripts\/release-candidate\.mjs/)
    assert.match(workflow, /--base-root "\$GITHUB_WORKSPACE\/base"/)
    assert.match(workflow, /repos\/\$GITHUB_REPOSITORY\/releases\/tags\/\$tag/)
    assert.match(workflow, /repos\/\$GITHUB_REPOSITORY\/git\/ref\/tags\/\$tag/)

    for (const forbidden of [
      /pnpm install/,
      /rust-toolchain/,
      /cargo (?:build|check|test)/,
      /tauri-action/,
      /gh release (?:create|edit|upload)/,
      /TAURI_SIGNING_PRIVATE_KEY/
    ]) {
      assert.doesNotMatch(workflow, forbidden)
    }
  })

  it('uses the GitHub App only for an idempotent protected patch pull request', async () => {
    const workflow = await readFile(new URL('../.github/workflows/release-pr.yml', import.meta.url), 'utf8')
    const patchCondition = "if: steps.release-decision.outputs.kind == 'patch-required'"

    assert.match(workflow, /id:\s*release-decision/)
    assert.match(workflow, /app-id:\s*\$\{\{ secrets\.POCKIVA_RELEASE_APP_ID \}\}/)
    assert.match(workflow, /private-key:\s*\$\{\{ secrets\.POCKIVA_RELEASE_APP_PRIVATE_KEY \}\}/)
    assert.match(workflow, /permission-contents:\s*write/)
    assert.match(workflow, /permission-pull-requests:\s*write/)
    assert.match(
      workflow,
      /uses:\s*actions\/create-github-app-token@bcd2ba49218906704ab6c1aa796996da409d3eb1\s*#\s*v3\.2\.0/
    )
    assert.equal(workflow.match(new RegExp(patchCondition.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'g'))?.length, 3)
    const liveValidationIndex = workflow.indexOf('- name: Revalidate live release pull request')
    const tokenIndex = workflow.indexOf('- name: Mint repository-scoped release App token')
    const mutationIndex = workflow.indexOf('- name: Create or reuse protected patch pull request')
    assert.ok(liveValidationIndex > 0)
    assert.ok(tokenIndex > liveValidationIndex)
    assert.ok(mutationIndex > tokenIndex)
    assert.match(workflow, /GH_TOKEN:\s*\$\{\{ github\.token \}\}/)
    assert.match(workflow, /gh api "repos\/\$GITHUB_REPOSITORY\/pulls\/\$RELEASE_PR_NUMBER"/)
    assert.match(workflow, /\.state == "open"/)
    assert.match(workflow, /\.base\.ref == "main"/)
    assert.match(workflow, /\.head\.ref == "develop"/)
    assert.match(workflow, /\.head\.repo\.full_name == \$repository/)
    assert.match(workflow, /\.head\.sha == \$head_sha/)
    assert.match(workflow, /automation\/release-pr-\$\{\{ github\.event\.pull_request\.number \}\}-patch/)
    assert.match(workflow, /node trusted\/scripts\/version-bump\.mjs patch/)
    assert.match(workflow, /--root "\$GITHUB_WORKSPACE\/candidate"/)
    assert.match(workflow, /gh pr create[\s\S]*--base develop/)
    assert.match(workflow, /gh pr merge[\s\S]*--auto[\s\S]*--squash[\s\S]*--delete-branch/)
    assert.match(workflow, /Remote automation branch exists without its expected open pull request/)
    assert.match(workflow, /Reusing protected patch pull request/)
    assert.match(workflow, /--state all/)
    assert.match(workflow, /state,mergedAt,closedAt/)
    assert.match(workflow, /Previously closed or merged automation pull request blocks safe reuse/)
    assert.equal(workflow.match(/block_release_until_patch_merges/g)?.length, 3)
    assert.match(workflow, /Patch pull request .* must merge into develop before this release candidate can pass/)
    const mutationStep = /- name: Create or reuse protected patch pull request([\s\S]*)$/.exec(workflow)?.[1]
    assert.ok(mutationStep)
    assert.doesNotMatch(mutationStep, /exit 0/)
    assert.match(workflow, /if \[\[ "\$decision_kind" == "valid" \]\]; then[\s\S]*echo "kind=valid"[\s\S]*exit 0/)

    for (const path of [
      'Cargo.lock',
      'Cargo.toml',
      'apps/desktop/package.json',
      'apps/desktop/src-tauri/tauri.conf.json'
    ]) {
      assert.match(workflow, new RegExp(path.replace(/[./]/g, '\\$&')))
    }

    assert.doesNotMatch(workflow, /push[^\n]*(develop|main)/)
    assert.doesNotMatch(workflow, /force-with-lease|--force/)
    assert.doesNotMatch(workflow, /node\s+candidate\//)

    const decisionStep = /- name: Validate versions and published tags([\s\S]*?)(?=\n\s+- name:)/.exec(workflow)?.[1]
    assert.ok(decisionStep)
    assert.doesNotMatch(decisionStep, /POCKIVA_RELEASE_APP|secrets\./)
  })

  it('pins current actions and reads Node.js from .tool-versions in every workflow', async () => {
    const [ciWorkflow, releaseWorkflow, releasePrWorkflow] = await Promise.all([
      readFile(new URL('../.github/workflows/ci.yml', import.meta.url), 'utf8'),
      readFile(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8'),
      readFile(new URL('../.github/workflows/release-pr.yml', import.meta.url), 'utf8')
    ])
    const workflows = [ciWorkflow, releaseWorkflow, releasePrWorkflow]
    const allowedActions = new Set([
      'actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1',
      'actions/create-github-app-token@bcd2ba49218906704ab6c1aa796996da409d3eb1',
      'actions/setup-node@820762786026740c76f36085b0efc47a31fe5020',
      'dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c',
      'pnpm/action-setup@0977fd99725f1db4007ccb2928dbb4e90d06cc86',
      'tauri-apps/tauri-action@1deb371b0cd8bd54025b384f1cd735e725c4060f'
    ])

    for (const workflow of workflows) {
      assert.doesNotMatch(workflow, /^\s*NODE_VERSION:/m)
      assert.doesNotMatch(workflow, /^\s*node-version:/m)

      const actionReferences = [...workflow.matchAll(/^\s*uses:\s*([^\s#]+)/gm)].map((match) => match[1])
      assert.ok(actionReferences.length > 0)
      assert.ok(
        actionReferences.every((reference) => allowedActions.has(reference)),
        'all actions must use approved SHAs'
      )

      const setupNodeCount = actionReferences.filter((reference) => reference.startsWith('actions/setup-node@')).length
      const nodeVersionFileCount =
        workflow.match(/^\s*node-version-file:\s*(?:trusted\/)?\.tool-versions\s*$/gm)?.length ?? 0
      assert.equal(nodeVersionFileCount, setupNodeCount)
    }
  })
})
