import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { compareStableVersions, loadRepositoryReleaseMetadata, validateReleaseMetadata } from './release-config.mjs'
import { bumpStableVersion } from './version-bump.mjs'

export const classifyReleaseCandidate = ({ baseVersion, candidateVersion }) => {
  const comparison = compareStableVersions(candidateVersion, baseVersion)
  if (comparison < 0) {
    throw new Error(`Release candidate version ${candidateVersion} must not be lower than base version ${baseVersion}`)
  }
  if (comparison > 0) {
    return { kind: 'valid', version: candidateVersion, tag: `v${candidateVersion}` }
  }

  const nextVersion = bumpStableVersion(baseVersion, 'patch')
  return {
    kind: 'patch-required',
    version: candidateVersion,
    tag: `v${candidateVersion}`,
    nextVersion,
    nextTag: `v${nextVersion}`
  }
}

export const parseReleaseCandidateArguments = (args) => {
  let baseRoot = null
  let candidateRoot = null
  let printJson = false

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index]
    if (argument === '--base-root' || argument === '--candidate-root') {
      const value = args[++index]
      if (!value || value.startsWith('--')) throw new Error(`${argument} requires a value`)
      if (argument === '--base-root') {
        if (baseRoot) throw new Error('--base-root may be provided only once')
        baseRoot = resolve(value)
      } else {
        if (candidateRoot) throw new Error('--candidate-root may be provided only once')
        candidateRoot = resolve(value)
      }
    } else if (argument === '--print-json') {
      if (printJson) throw new Error('--print-json may be provided only once')
      printJson = true
    } else {
      throw new Error(`Unknown release candidate argument: ${argument}`)
    }
  }

  if (!baseRoot) throw new Error('--base-root is required')
  if (!candidateRoot) throw new Error('--candidate-root is required')
  return { baseRoot, candidateRoot, printJson }
}

const run = async () => {
  const { baseRoot, candidateRoot, printJson } = parseReleaseCandidateArguments(process.argv.slice(2))
  const [baseMetadata, candidateMetadata] = await Promise.all([
    loadRepositoryReleaseMetadata(baseRoot),
    loadRepositoryReleaseMetadata(candidateRoot)
  ])
  const baseVersion = validateReleaseMetadata(baseMetadata)
  const candidateVersion = validateReleaseMetadata(candidateMetadata)
  const decision = classifyReleaseCandidate({ baseVersion, candidateVersion })

  console.log(printJson ? JSON.stringify(decision) : `Release candidate ${candidateVersion}: ${decision.kind}`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  run().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
