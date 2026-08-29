import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { after, describe, it } from 'node:test'
import { fileURLToPath } from 'node:url'
import { classifyReleaseCandidate } from './release-candidate.mjs'

const releaseCandidateScript = fileURLToPath(new URL('./release-candidate.mjs', import.meta.url))

const fixtureFiles = (version = '0.1.0') =>
  new Map([
    [
      'apps/desktop/src-tauri/tauri.conf.json',
      `${JSON.stringify(
        {
          productName: 'Pockiva',
          version,
          identifier: 'com.pedro.pockiva',
          bundle: { createUpdaterArtifacts: true },
          plugins: {
            updater: {
              pubkey: 'public-key',
              endpoints: ['https://github.com/pedrocs378/pockiva/releases/latest/download/latest.json']
            }
          }
        },
        null,
        2
      )}\n`
    ],
    ['apps/desktop/package.json', `${JSON.stringify({ name: '@gameboy/desktop', version }, null, 2)}\n`],
    ['Cargo.toml', `[workspace]\n\n[workspace.package]\nversion = "${version}"\n`],
    [
      'apps/desktop/src-tauri/capabilities/default.json',
      `${JSON.stringify({ permissions: ['process:default', 'updater:default'] }, null, 2)}\n`
    ],
    ['apps/desktop/vite.config.ts', "export default { envPrefix: ['VITE_'] }\n"]
  ])

const createFixture = async (files = fixtureFiles()) => {
  const root = await mkdtemp(join(tmpdir(), 'pockiva-release-candidate-'))
  for (const [relativePath, contents] of files) {
    const path = join(root, relativePath)
    await mkdir(dirname(path), { recursive: true })
    await writeFile(path, contents)
  }
  return root
}

describe('release candidate', () => {
  const directories = []
  after(async () => Promise.all(directories.map((directory) => rm(directory, { recursive: true, force: true }))))

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

  it('prints exactly one JSON decision for valid repository roots', async () => {
    const baseRoot = await createFixture(fixtureFiles('0.1.0'))
    const candidateRoot = await createFixture(fixtureFiles('0.1.1'))
    directories.push(baseRoot, candidateRoot)

    const output = execFileSync(
      process.execPath,
      [releaseCandidateScript, '--base-root', baseRoot, '--candidate-root', candidateRoot, '--print-json'],
      { encoding: 'utf8' }
    )

    assert.equal(output, '{"kind":"valid","version":"0.1.1","tag":"v0.1.1"}\n')
  })

  it('rejects divergent candidate metadata before emitting JSON', async () => {
    const baseRoot = await createFixture(fixtureFiles('0.1.0'))
    const candidateFiles = fixtureFiles('0.1.1')
    candidateFiles.set(
      'apps/desktop/package.json',
      `${JSON.stringify({ name: '@gameboy/desktop', version: '0.2.0' }, null, 2)}\n`
    )
    const candidateRoot = await createFixture(candidateFiles)
    directories.push(baseRoot, candidateRoot)

    const result = spawnSync(
      process.execPath,
      [releaseCandidateScript, '--base-root', baseRoot, '--candidate-root', candidateRoot, '--print-json'],
      { encoding: 'utf8' }
    )

    assert.notEqual(result.status, 0)
    assert.equal(result.stdout, '')
    assert.match(result.stderr, /must match/)
  })
})
