export type PackageManager = 'npm' | 'pnpm' | 'yarn' | 'bun'
export type Ecosystem = 'none' | 'javascript'
export type BuildConfig = {
  deploymentType: 'static'
  ecosystem: Ecosystem
  packageManager: PackageManager | null
  framework: 'plainHtml' | 'vite'
}

export function buildPreset(ecosystem: Ecosystem, packageManager: PackageManager = 'npm') {
  if (ecosystem === 'none') {
    return {
      buildConfig: {
        deploymentType: 'static',
        ecosystem,
        packageManager: null,
        framework: 'plainHtml',
      } as BuildConfig,
      installCommand: '',
      buildCommand: '',
      outputDirectory: '.',
    }
  }
  const installCommands: Record<PackageManager, string> = {
    npm: 'npm ci',
    pnpm: 'pnpm install --frozen-lockfile',
    yarn: 'yarn install --immutable',
    bun: 'bun install --frozen-lockfile',
  }
  return {
    buildConfig: {
      deploymentType: 'static',
      ecosystem,
      packageManager,
      framework: 'vite',
    } as BuildConfig,
    installCommand: installCommands[packageManager],
    buildCommand: `${packageManager} run build`,
    outputDirectory: 'dist',
  }
}
