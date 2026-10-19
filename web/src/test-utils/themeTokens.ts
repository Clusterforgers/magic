/** The design tokens of index.css as tests read them: the variables of the light and the dark theme, resolved to colours. */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { parseOklch, toSrgb } from './color'
import type { Mapping, Rgb } from './color'
import { blockAfter } from './cssBlocks'

/** The two themes of the app. */
export type Theme = 'light' | 'dark'

/** The themes, light first. */
export const THEMES: readonly Theme[] = ['light', 'dark']

/** The two ways a colour outside the sRGB gamut is brought in; a token must pass with both. */
export const MAPPINGS: readonly Mapping[] = ['clip', 'chroma']

/** The text of `index.css`, read as text rather than imported, which would hand back what Tailwind builds from it. */
export const STYLESHEET = readFileSync(join(import.meta.dirname, '../index.css'), 'utf8')

/** The custom properties a block declares, by name without the dashes. */
export function declarations(block: string): Map<string, string> {
  const found = new Map<string, string>()
  for (const match of block.matchAll(/(?:^|[\s;])--([a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    found.set(match[1] ?? '', (match[2] ?? '').trim())
  }
  return found
}

// The heads include the line start: the first ".dark" of the file is the `@custom-variant`, not the block.
/** The variables `:root` sets, which are the light theme's. */
export const ROOT = declarations(blockAfter(STYLESHEET, '\n:root {'))
/** The variables `.dark` sets over the light ones. */
export const DARK = declarations(blockAfter(STYLESHEET, '\n.dark {'))
/** The theme mapping that turns variables into Tailwind utilities. */
export const THEME = declarations(blockAfter(STYLESHEET, '@theme inline'))
/** Every variable in force in a theme. */
export const VARS: Record<Theme, ReadonlyMap<string, string>> = {
  light: ROOT,
  dark: new Map([...ROOT, ...DARK]),
}

/** The value of a variable in a theme, following `var(--other)` aliases to the end. */
export function valueOf(vars: ReadonlyMap<string, string>, name: string): string {
  const value = vars.get(name)
  if (value === undefined) throw new Error(`--${name} is not defined`)
  const alias = /^var\(--([a-z0-9-]+)\)$/.exec(value)
  return alias === null ? value : valueOf(vars, alias[1] ?? '')
}

/** The sRGB colour of a token in a theme, an opaque one. */
export function paint(theme: Theme, token: string, mapping: Mapping): Rgb {
  const color = parseOklch(valueOf(VARS[theme], token))
  if (color.alpha !== 1) throw new Error(`--${token} is translucent`)
  return toSrgb(color, mapping)
}
