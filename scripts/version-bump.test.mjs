import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { after, describe, it } from 'node:test'
import { fileURLToPath } from 'node:url'
import { applyVersionBump, bumpStableVersion, parseVersionBumpArguments } from './version-bump.mjs'

const versionFiles = ['apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/package.json', 'Cargo.toml', 'Cargo.lock']
const versionBumpScript = fileURLToPath(new URL('./version-bump.mjs', import.meta.url))

const fixtureFiles = (version = '0.1.0') =>
  new Map([
    [
      'apps/desktop/src-tauri/tauri.conf.json',
      `${JSON.stringify(
        {
          productName: 'Pockiva',
          version,
          identifier: 'com.pedro.pockiva',
          bundle: { createUpdaterArtifacts: true, icon: ['one.png', 'two.png'] },
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
    [
      'Cargo.toml',
      `[workspace]\nmembers = ["apps/desktop/src-tauri", "crates/gb-core", "crates/gb-network"]\n\n[workspace.package]\nedition = "2024"\nversion = "${version}"\n\n[workspace.lints.rust]\nunsafe_code = "forbid"\n`
    ],
    [
      'Cargo.lock',
      `version = 4\n\n[[package]]\nname = "external-package"\nversion = "${version}"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n\n[[package]]\nname = "gameboy-desktop"\nversion = "${version}"\ndependencies = [\n "gb-core",\n "gb-network",\n]\n\n[[package]]\nname = "gb-core"\nversion = "${version}"\n\n[[package]]\nname = "gb-network"\nversion = "${version}"\ndependencies = [\n "gb-core",\n]\n`
    ],
    [
      'apps/desktop/src-tauri/capabilities/default.json',
      `${JSON.stringify({ permissions: ['process:default', 'updater:default'] }, null, 2)}\n`
    ],
    ['apps/desktop/vite.config.ts', "export default { envPrefix: ['VITE_'] }\n"]
  ])

const createFixture = async (files = fixtureFiles()) => {
  const root = await mkdtemp(join(tmpdir(), 'pockiva-version-bump-'))
  for (const [relativePath, contents] of files) {
    const path = join(root, relativePath)
    await mkdir(join(path, '..'), { recursive: true })
    await writeFile(path, contents)
  }
  return root
}

const readFixtureFiles = async (root, paths) =>
  new Map(await Promise.all(paths.map(async (path) => [path, await readFile(join(root, path), 'utf8')])))

describe('version bump', () => {
  const directories = []
  after(async () => Promise.all(directories.map((directory) => rm(directory, { recursive: true, force: true }))))

  it('calculates patch, minor, and major versions', () => {
    assert.equal(bumpStableVersion('0.1.9', 'patch'), '0.1.10')
    assert.equal(bumpStableVersion('0.1.9', 'minor'), '0.2.0')
    assert.equal(bumpStableVersion('0.1.9', 'major'), '1.0.0')
    assert.throws(() => bumpStableVersion('0.1.9', 'feature'), /patch, minor, or major/)
    assert.throws(() => bumpStableVersion('0.2.0-beta.1', 'patch'), /stable SemVer/)
  })

  it('updates exactly the published manifests and inherited local Cargo lock packages', async () => {
    const root = await createFixture()
    directories.push(root)
    const allPaths = [...fixtureFiles().keys()]
    const before = await readFixtureFiles(root, allPaths)

    assert.equal(await applyVersionBump({ root, kind: 'patch' }), '0.1.1')

    const afterFiles = await readFixtureFiles(root, allPaths)
    assert.deepEqual(
      allPaths.filter((path) => before.get(path) !== afterFiles.get(path)).sort(),
      [...versionFiles].sort()
    )
    const tauriText = afterFiles.get('apps/desktop/src-tauri/tauri.conf.json')
    assert.equal(JSON.parse(tauriText).version, '0.1.1')
    assert.match(tauriText, /"icon": \["one\.png", "two\.png"\]/)
    assert.match(
      tauriText,
      /"endpoints": \["https:\/\/github\.com\/pedrocs378\/pockiva\/releases\/latest\/download\/latest\.json"\]/
    )
    assert.equal(JSON.parse(afterFiles.get('apps/desktop/package.json')).version, '0.1.1')
    assert.match(afterFiles.get('Cargo.toml'), /\[workspace\.package\][\s\S]*version = "0\.1\.1"/)

    const lockfile = afterFiles.get('Cargo.lock')
    for (const name of ['gameboy-desktop', 'gb-core', 'gb-network']) {
      assert.match(lockfile, new RegExp(`name = "${name}"\\nversion = "0\\.1\\.1"`))
    }
    assert.match(lockfile, /name = "external-package"\nversion = "0\.1\.0"/)
  })

  it('rejects divergent starting versions without changing any file', async () => {
    const files = fixtureFiles()
    files.set(
      'apps/desktop/package.json',
      `${JSON.stringify({ name: '@gameboy/desktop', version: '0.2.0' }, null, 2)}\n`
    )
    const root = await createFixture(files)
    directories.push(root)
    const before = await readFixtureFiles(root, versionFiles)

    await assert.rejects(() => applyVersionBump({ root, kind: 'patch' }), /must match/)
    assert.deepEqual(await readFixtureFiles(root, versionFiles), before)
  })

  it('rejects a malformed lockfile and leaves every original file byte-for-byte unchanged', async () => {
    const files = fixtureFiles()
    files.set('Cargo.lock', files.get('Cargo.lock').replace('name = "gb-network"', 'name = "wrong-package"'))
    const root = await createFixture(files)
    directories.push(root)
    const before = await readFixtureFiles(root, versionFiles)

    await assert.rejects(() => applyVersionBump({ root, kind: 'patch' }), /exactly one Cargo\.lock package block/)
    assert.deepEqual(await readFixtureFiles(root, versionFiles), before)
  })

  it('restores all version files byte-for-byte when a later rename fails once', async () => {
    const root = await createFixture()
    directories.push(root)
    const before = await readFixtureFiles(root, versionFiles)
    const renameFailure = new Error('injected second rename failure')
    let renameCalls = 0
    const fileSystemOperations = {
      rename: async (...arguments_) => {
        renameCalls += 1
        if (renameCalls === 2) throw renameFailure
        await rename(...arguments_)
      }
    }

    await assert.rejects(
      () => applyVersionBump({ root, kind: 'patch', fileSystemOperations }),
      (error) => error === renameFailure
    )
    assert.equal(renameCalls, 6)
    assert.deepEqual(await readFixtureFiles(root, versionFiles), before)
  })

  it('preserves the original and rollback failures when restoration also fails', async () => {
    const root = await createFixture()
    directories.push(root)
    const renameFailure = new Error('injected second rename failure')
    const rollbackFailure = new Error('injected rollback rename failure')
    let renameCalls = 0
    const fileSystemOperations = {
      rename: async (...arguments_) => {
        renameCalls += 1
        if (renameCalls === 2) throw renameFailure
        if (renameCalls === 3) throw rollbackFailure
        await rename(...arguments_)
      }
    }

    await assert.rejects(
      () => applyVersionBump({ root, kind: 'patch', fileSystemOperations }),
      (error) => {
        assert.ok(error instanceof AggregateError)
        assert.deepEqual(error.errors, [renameFailure, rollbackFailure])
        assert.equal(error.cause, renameFailure)
        return true
      }
    )
  })

  it('prints a raw workflow value only when --print-version is present', async () => {
    const rawRoot = await createFixture()
    const humanRoot = await createFixture()
    directories.push(rawRoot, humanRoot)

    const rawOutput = execFileSync(
      process.execPath,
      [versionBumpScript, 'patch', '--root', rawRoot, '--print-version'],
      { encoding: 'utf8' }
    )
    const humanOutput = execFileSync(process.execPath, [versionBumpScript, 'patch', '--root', humanRoot], {
      encoding: 'utf8'
    })

    assert.equal(rawOutput, '0.1.1\n')
    assert.equal(humanOutput, 'Bumped Pockiva to 0.1.1\n')
  })

  it('rejects malformed CLI arguments before applying a bump', () => {
    assert.throws(() => parseVersionBumpArguments(['feature']), /patch, minor, or major/)
    assert.throws(() => parseVersionBumpArguments(['patch', '--root']), /requires a value/)
    assert.throws(() => parseVersionBumpArguments(['patch', 'extra']), /Unexpected positional argument/)
    assert.throws(() => parseVersionBumpArguments(['patch', '--root', '/tmp/one', '--root', '/tmp/two']), /only once/)
    assert.throws(() => parseVersionBumpArguments(['patch', '--print-version', '--print-version']), /only once/)
  })
})
