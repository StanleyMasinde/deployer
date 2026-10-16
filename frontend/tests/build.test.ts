import { expect, it } from 'vitest'
import { buildPreset, type PackageManager } from '../src/build'

it('plain static sites publish the repository root without build commands', () => {
  expect(buildPreset('none')).toEqual({
    buildConfig: {
      deploymentType: 'static',
      ecosystem: 'none',
      packageManager: null,
      framework: 'plainHtml',
    },
    installCommand: '',
    buildCommand: '',
    outputDirectory: '.',
  })
})

it.each(['npm', 'pnpm', 'yarn', 'bun'] as PackageManager[])(
  'Vite presets use the chosen %s package manager',
  (manager) => {
    const preset = buildPreset('javascript', manager)
    expect(preset.buildConfig).toEqual({
      deploymentType: 'static',
      ecosystem: 'javascript',
      packageManager: manager,
      framework: 'vite',
    })
    expect(preset.installCommand.startsWith(`${manager} `)).toBe(true)
    expect(preset.buildCommand).toBe(`${manager} run build`)
    expect(preset.outputDirectory).toBe('dist')
  },
)
