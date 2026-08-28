import { execFileSync } from 'node:child_process'
import { readdir, readFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const stableSemver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/
const updaterEndpoint = 'https://github.com/pedrocs378/pockiva/releases/latest/download/latest.json'
const privateKeyMarkers = [
  'TAURI_SIGNING_PRIVATE_KEY=',
  'untrusted comment: minisign encrypted secret key',
  'untrusted comment: minisign secret key'
]

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

const cargoWorkspaceVersion = (cargoManifest) => {
  const section = /\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/.exec(cargoManifest)?.[1]
  const version = section ? /^version\s*=\s*"([^"]+)"\s*$/m.exec(section)?.[1] : null
  if (!version) throw new Error('Cargo.toml must define workspace.package.version')
  return version
}

export const loadRepositoryReleaseMetadata = async (root = repositoryRoot) => {
  const [tauriText, desktopText, cargoText, capabilitiesText, viteConfig] = await Promise.all([
    readFile(join(root, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'),
    readFile(join(root, 'apps/desktop/package.json'), 'utf8'),
    readFile(join(root, 'Cargo.toml'), 'utf8'),
    readFile(join(root, 'apps/desktop/src-tauri/capabilities/default.json'), 'utf8'),
    readFile(join(root, 'apps/desktop/vite.config.ts'), 'utf8')
  ])

  return {
    tauri: JSON.parse(tauriText),
    desktopPackage: JSON.parse(desktopText),
    cargoVersion: cargoWorkspaceVersion(cargoText),
    capabilities: JSON.parse(capabilitiesText),
    viteConfig
  }
}

export const validateReleaseMetadata = ({ tauri, desktopPackage, cargoVersion, capabilities, viteConfig }) => {
  const version = tauri.version
  parseStableVersion(version, 'tauri.conf.json version')

  if (desktopPackage.version !== version || cargoVersion !== version) {
    throw new Error(
      `Desktop package (${desktopPackage.version}) and Cargo (${cargoVersion}) versions must match Tauri (${version})`
    )
  }
  if (tauri.productName !== 'Pockiva' || tauri.identifier !== 'com.pedro.pockiva') {
    throw new Error('Tauri productName and identifier must use the Pockiva release identity')
  }
  if (tauri.bundle?.createUpdaterArtifacts !== true) {
    throw new Error('Tauri bundle.createUpdaterArtifacts must be true')
  }

  const updater = tauri.plugins?.updater
  if (!updater?.pubkey?.trim() || updater.endpoints?.length !== 1 || updater.endpoints[0] !== updaterEndpoint) {
    throw new Error('Tauri updater must use the committed public key and canonical GitHub latest.json endpoint')
  }

  const permissions = new Set(capabilities.permissions)
  if (!permissions.has('updater:default') || !permissions.has('process:default')) {
    throw new Error('Desktop capabilities must grant updater:default and process:default')
  }
  if (/envPrefix\s*:\s*\[[^\]]*['"]TAURI_/s.test(viteConfig)) {
    throw new Error('Vite envPrefix must never expose TAURI_ variables to the frontend')
  }

  return version
}

export const assertVersionBump = (current, previous) => {
  if (compareStableVersions(current, previous) <= 0) {
    throw new Error(`Release version ${current} must be greater than main version ${previous}`)
  }
}

const filesUnder = async (directory) => {
  const entries = await readdir(directory, { withFileTypes: true })
  const nested = await Promise.all(
    entries.map((entry) => {
      const path = join(directory, entry.name)
      return entry.isDirectory() ? filesUnder(path) : [path]
    })
  )
  return nested.flat()
}

export const assertBundleHasNoSigningKey = async (directory, privateKey = '') => {
  const markers = [...privateKeyMarkers, privateKey].filter(Boolean)
  for (const path of await filesUnder(directory)) {
    const contents = await readFile(path)
    if (markers.some((marker) => contents.includes(Buffer.from(marker)))) {
      throw new Error(`Frontend bundle contains private signing material: ${path}`)
    }
  }
}

export const validateReleaseWorkflow = async (root = repositoryRoot) => {
  const [workflow, releasePrWorkflow] = await Promise.all([
    readFile(join(root, '.github/workflows/release.yml'), 'utf8'),
    readFile(join(root, '.github/workflows/release-pr.yml'), 'utf8')
  ])
  const requirements = [
    ['closed pull request trigger', /pull_request:[\s\S]*branches:\s*\[main\][\s\S]*types:\s*\[closed\]/],
    ['merged guard', /github\.event\.pull_request\.merged\s*==\s*true/],
    ['develop head guard', /github\.event\.pull_request\.head\.ref\s*==\s*'develop'/],
    ['same-repository guard', /github\.event\.pull_request\.head\.repo\.full_name\s*==\s*github\.repository/],
    ['non-canceling release concurrency', /cancel-in-progress:\s*false/],
    ['release environment', /environment:\s*release/],
    ['draft aggregation', /releaseDraft:\s*true/],
    ['generated release notes', /generateReleaseNotes:\s*true/],
    ['updater JSON upload', /uploadUpdaterJson:\s*true/],
    ['NSIS updater preference', /updaterJsonPreferNsis:\s*true/],
    ['immutable action revisions', /uses:\s*[^\s]+@[0-9a-f]{40}/]
  ]
  for (const [label, pattern] of requirements) {
    if (!pattern.test(workflow)) throw new Error(`Release workflow is missing ${label}`)
  }

  const releasePrRequirements = [
    [
      'main pull_request_target trigger',
      /pull_request_target:[\s\S]*branches:\s*\[main\][\s\S]*types:\s*\[opened, reopened, synchronize\]/
    ],
    ['single release candidate check', /^\s{4}name:\s*Validate release candidate\s*$/m],
    ['develop source guard', /\[\[ "\$HEAD_REF" != "develop" \]\]/],
    ['same-repository source guard', /\[\[ "\$HEAD_REPOSITORY" != "\$GITHUB_REPOSITORY" \]\]/],
    ['read-only permissions', /^permissions:\n {2}contents: read\n {2}pull-requests: read$/m],
    ['pull-request concurrency', /group:\s*release-pr-\$\{\{ github\.event\.pull_request\.number \}\}/],
    ['canceling release candidate concurrency', /cancel-in-progress:\s*true/],
    ['trusted base checkout', /ref:\s*\$\{\{ github\.event\.pull_request\.base\.sha \}\}[\s\S]*path:\s*trusted/],
    [
      'isolated candidate checkout',
      /ref:\s*\$\{\{ github\.event\.pull_request\.head\.sha \}\}[\s\S]*path:\s*candidate/
    ],
    ['trusted release classifier', /node trusted\/scripts\/release-candidate\.mjs/],
    ['trusted Node.js version file', /node-version-file:\s*trusted\/\.tool-versions/]
  ]
  for (const [label, pattern] of releasePrRequirements) {
    if (!pattern.test(releasePrWorkflow)) throw new Error(`Release pull-request gate is missing ${label}`)
  }
  if (/^\s*[^#\n]+:\s*write\s*$/m.test(releasePrWorkflow)) {
    throw new Error('Release pull-request gate must keep read-only permissions')
  }
  if ((releasePrWorkflow.match(/^\s{4}runs-on:/gm)?.length ?? 0) !== 1) {
    throw new Error('Release pull-request gate must define exactly one job')
  }

  const forbiddenReleasePrPatterns = [
    /pnpm install/,
    /rust-toolchain/,
    /cargo (?:build|check|test)/,
    /tauri-action/,
    /gh release (?:create|edit|upload)/,
    /TAURI_SIGNING_PRIVATE_KEY/,
    /POCKIVA_RELEASE_APP/,
    /secrets\./
  ]
  if (forbiddenReleasePrPatterns.some((pattern) => pattern.test(releasePrWorkflow))) {
    throw new Error('Release pull-request gate must remain read-only and must not build or use release secrets')
  }
}

const versionFromRef = (ref) => {
  if (!/^[0-9a-f]{40}$/.test(ref)) throw new Error('Previous release ref must be a full commit SHA')
  const contents = execFileSync('git', ['show', `${ref}:apps/desktop/src-tauri/tauri.conf.json`], {
    cwd: repositoryRoot,
    encoding: 'utf8'
  })
  return JSON.parse(contents).version
}

const run = async () => {
  const args = process.argv.slice(2)
  let printVersion = false
  let checkBundle = false
  let previousRef = null

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index]
    if (argument === '--print-version') printVersion = true
    else if (argument === '--check-bundle') checkBundle = true
    else if (argument === '--previous-ref') previousRef = args[++index] ?? null
    else throw new Error(`Unknown release configuration argument: ${argument}`)
  }

  const version = validateReleaseMetadata(await loadRepositoryReleaseMetadata())
  await validateReleaseWorkflow()
  if (previousRef) assertVersionBump(version, versionFromRef(previousRef))
  if (checkBundle) {
    await assertBundleHasNoSigningKey(join(repositoryRoot, 'apps/desktop/dist'), process.env.TAURI_SIGNING_PRIVATE_KEY)
  }

  console.log(printVersion ? version : `Release configuration valid for v${version}`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  run().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
