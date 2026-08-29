import { randomUUID } from 'node:crypto'
import { readFile, rename, rm, writeFile } from 'node:fs/promises'
import { basename, dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { loadRepositoryReleaseMetadata, parseStableVersion, validateReleaseMetadata } from './release-config.mjs'

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const acceptedKinds = new Set(['patch', 'minor', 'major'])
const localCargoPackages = ['gameboy-desktop', 'gb-core', 'gb-network']
const versionFilePaths = [
  'apps/desktop/src-tauri/tauri.conf.json',
  'apps/desktop/package.json',
  'Cargo.toml',
  'Cargo.lock'
]
const defaultFileSystemOperations = { rename, rm, writeFile }

export const bumpStableVersion = (version, kind) => {
  if (!acceptedKinds.has(kind)) throw new Error('Version kind must be patch, minor, or major')
  const [major, minor, patch] = parseStableVersion(version, 'Current version')
  if (kind === 'major') return `${major + 1}.0.0`
  if (kind === 'minor') return `${major}.${minor + 1}.0`
  return `${major}.${minor}.${patch + 1}`
}

export const loadVersionFileTexts = async (root = repositoryRoot) =>
  new Map(
    await Promise.all(
      versionFilePaths.map(async (relativePath) => {
        const path = join(root, relativePath)
        return [path, await readFile(path, 'utf8')]
      })
    )
  )

const findVersionFile = (texts, relativePath) => {
  const suffix = join('', relativePath)
  const matches = [...texts.keys()].filter((path) => path.endsWith(suffix))
  if (matches.length !== 1) throw new Error(`Expected exactly one loaded version file for ${relativePath}`)
  return matches[0]
}

const compactPrimitiveJsonArrays = (serialized) =>
  serialized.replace(
    /^(\s*)("[^"\n]+": )\[\n((?:[^\n]*\n)*?)\1\](,?)$/gm,
    (match, indentation, property, elements, trailingComma) => {
      try {
        const values = JSON.parse(`[${elements.trim()}]`)
        if (!values.every((value) => value === null || ['boolean', 'number', 'string'].includes(typeof value))) {
          return match
        }
        const candidate = `${indentation}${property}[${values.map((value) => JSON.stringify(value)).join(', ')}]${trailingComma}`
        return candidate.length <= 120 ? candidate : match
      } catch {
        return match
      }
    }
  )

const rewriteJsonVersion = (text, currentVersion, nextVersion, label) => {
  const document = JSON.parse(text)
  if (!document || typeof document !== 'object' || Array.isArray(document)) {
    throw new Error(`${label} must contain a JSON object`)
  }
  if (document.version !== currentVersion) {
    throw new Error(`${label} version must equal validated version ${currentVersion}`)
  }
  document.version = nextVersion
  return `${compactPrimitiveJsonArrays(JSON.stringify(document, null, 2))}\n`
}

const rewriteCargoWorkspaceVersion = (text, currentVersion, nextVersion) => {
  const sectionMatch = /\[workspace\.package\]([\s\S]*?)(?=\n\[|$)/.exec(text)
  if (!sectionMatch) throw new Error('Cargo.toml must contain exactly one [workspace.package] section')
  if ((text.match(/^\[workspace\.package\]\s*$/gm) ?? []).length !== 1) {
    throw new Error('Cargo.toml must contain exactly one [workspace.package] section')
  }

  const section = sectionMatch[1]
  const assignments = [...section.matchAll(/^version\s*=\s*"([^"]+)"\s*$/gm)]
  if (assignments.length !== 1) {
    throw new Error('Cargo.toml [workspace.package] must contain exactly one version assignment')
  }
  if (assignments[0][1] !== currentVersion) {
    throw new Error(`Cargo.toml workspace version must equal validated version ${currentVersion}`)
  }

  const rewrittenSection = section.replace(/^version\s*=\s*"[^"]+"\s*$/m, (assignment) =>
    assignment.replace(`"${currentVersion}"`, `"${nextVersion}"`)
  )
  return `${text.slice(0, sectionMatch.index)}[workspace.package]${rewrittenSection}${text.slice(
    sectionMatch.index + sectionMatch[0].length
  )}`
}

const cargoPackageBlocks = (text) => {
  const starts = [...text.matchAll(/^\[\[package\]\]\s*$/gm)].map((match) => match.index)
  return starts.map((start, index) => ({
    start,
    end: starts[index + 1] ?? text.length,
    text: text.slice(start, starts[index + 1] ?? text.length)
  }))
}

const rewriteCargoLockVersions = (text, currentVersion, nextVersion) => {
  const replacements = []
  const blocks = cargoPackageBlocks(text)

  for (const packageName of localCargoPackages) {
    const matchingBlocks = blocks.filter((block) => /^name\s*=\s*"([^"]+)"\s*$/m.exec(block.text)?.[1] === packageName)
    if (matchingBlocks.length !== 1) {
      throw new Error(`Cargo.lock must contain exactly one Cargo.lock package block for ${packageName}`)
    }

    const block = matchingBlocks[0]
    const versions = [...block.text.matchAll(/^version\s*=\s*"([^"]+)"\s*$/gm)]
    if (versions.length !== 1 || versions[0][1] !== currentVersion) {
      throw new Error(`Cargo.lock package ${packageName} must inherit version ${currentVersion}`)
    }
    replacements.push({
      start: block.start,
      end: block.end,
      text: block.text.replace(/^version\s*=\s*"[^"]+"\s*$/m, (assignment) =>
        assignment.replace(`"${currentVersion}"`, `"${nextVersion}"`)
      )
    })
  }

  let rewritten = text
  for (const replacement of replacements.sort((left, right) => right.start - left.start)) {
    rewritten = `${rewritten.slice(0, replacement.start)}${replacement.text}${rewritten.slice(replacement.end)}`
  }
  return rewritten
}

export const rewriteVersionFileTexts = ({ texts, currentVersion, nextVersion }) => {
  parseStableVersion(currentVersion, 'Current version')
  parseStableVersion(nextVersion, 'Next version')
  const rewritten = new Map(texts)
  const tauriPath = findVersionFile(texts, versionFilePaths[0])
  const desktopPath = findVersionFile(texts, versionFilePaths[1])
  const cargoManifestPath = findVersionFile(texts, versionFilePaths[2])
  const cargoLockPath = findVersionFile(texts, versionFilePaths[3])

  rewritten.set(tauriPath, rewriteJsonVersion(texts.get(tauriPath), currentVersion, nextVersion, 'tauri.conf.json'))
  rewritten.set(
    desktopPath,
    rewriteJsonVersion(texts.get(desktopPath), currentVersion, nextVersion, 'apps/desktop/package.json')
  )
  rewritten.set(
    cargoManifestPath,
    rewriteCargoWorkspaceVersion(texts.get(cargoManifestPath), currentVersion, nextVersion)
  )
  rewritten.set(cargoLockPath, rewriteCargoLockVersions(texts.get(cargoLockPath), currentVersion, nextVersion))

  rewriteCargoWorkspaceVersion(rewritten.get(cargoManifestPath), nextVersion, currentVersion)
  rewriteCargoLockVersions(rewritten.get(cargoLockPath), nextVersion, currentVersion)
  return rewritten
}

const writeTextsAtomically = async (texts, fileSystemOverrides = {}) => {
  const fileSystemOperations = { ...defaultFileSystemOperations, ...fileSystemOverrides }
  const temporaryFiles = []
  try {
    for (const [path, contents] of texts) {
      const temporaryPath = join(dirname(path), `.${basename(path)}.${process.pid}.${randomUUID()}.tmp`)
      await fileSystemOperations.writeFile(temporaryPath, contents, 'utf8')
      temporaryFiles.push([temporaryPath, path])
    }
    for (const [temporaryPath, path] of temporaryFiles) await fileSystemOperations.rename(temporaryPath, path)
  } finally {
    await Promise.all(temporaryFiles.map(([temporaryPath]) => fileSystemOperations.rm(temporaryPath, { force: true })))
  }
}

export const restoreVersionFiles = async (originalTexts, fileSystemOperations = {}) =>
  writeTextsAtomically(originalTexts, fileSystemOperations)

export const writeVersionFilesTransaction = async ({ originalTexts, nextTexts, fileSystemOperations = {} }) => {
  if (originalTexts.size !== nextTexts.size || [...originalTexts.keys()].some((path) => !nextTexts.has(path))) {
    throw new Error('Version file transaction must preserve the complete destination set')
  }
  await writeTextsAtomically(nextTexts, fileSystemOperations)
}

export const applyVersionBump = async ({ root = repositoryRoot, kind, fileSystemOperations }) => {
  const resolvedRoot = resolve(root)
  const currentVersion = validateReleaseMetadata(await loadRepositoryReleaseMetadata(resolvedRoot))
  const nextVersion = bumpStableVersion(currentVersion, kind)
  const originalTexts = await loadVersionFileTexts(resolvedRoot)
  const nextTexts = rewriteVersionFileTexts({ texts: originalTexts, currentVersion, nextVersion })

  try {
    await writeVersionFilesTransaction({ originalTexts, nextTexts, fileSystemOperations })
    const writtenTexts = await loadVersionFileTexts(resolvedRoot)
    for (const [path, expected] of nextTexts) {
      if (writtenTexts.get(path) !== expected) throw new Error(`Version file was not written atomically: ${path}`)
    }
    const writtenVersion = validateReleaseMetadata(await loadRepositoryReleaseMetadata(resolvedRoot))
    if (writtenVersion !== nextVersion) {
      throw new Error(`Expected written version ${nextVersion}, received ${writtenVersion}`)
    }
  } catch (error) {
    try {
      await restoreVersionFiles(originalTexts, fileSystemOperations)
    } catch (rollbackError) {
      throw new AggregateError([error, rollbackError], 'Version bump failed and rollback also failed', { cause: error })
    }
    throw error
  }

  return nextVersion
}

export const parseVersionBumpArguments = (args) => {
  const [kind, ...options] = args
  if (!acceptedKinds.has(kind)) throw new Error('Version kind must be patch, minor, or major')

  let root = repositoryRoot
  let rootProvided = false
  let printVersion = false
  for (let index = 0; index < options.length; index += 1) {
    const argument = options[index]
    if (argument === '--root') {
      if (rootProvided) throw new Error('--root may be provided only once')
      const value = options[++index]
      if (!value || value.startsWith('--')) throw new Error('--root requires a value')
      root = resolve(value)
      rootProvided = true
    } else if (argument === '--print-version') {
      if (printVersion) throw new Error('--print-version may be provided only once')
      printVersion = true
    } else if (argument.startsWith('--')) {
      throw new Error(`Unknown version bump argument: ${argument}`)
    } else {
      throw new Error(`Unexpected positional argument: ${argument}`)
    }
  }

  return { kind, root, printVersion }
}

const run = async () => {
  const { root, kind, printVersion } = parseVersionBumpArguments(process.argv.slice(2))
  const version = await applyVersionBump({ root, kind })
  console.log(printVersion ? version : `Bumped Pockiva to ${version}`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  run().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
