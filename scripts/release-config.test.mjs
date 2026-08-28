import assert from 'node:assert/strict'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { after, describe, it } from 'node:test'
import {
  assertBundleHasNoSigningKey,
  assertVersionBump,
  loadRepositoryReleaseMetadata,
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
    assert.equal(validateReleaseMetadata(metadata), '0.1.0')
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
})
