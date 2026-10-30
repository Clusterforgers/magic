// Goal: every mark badge takes its colour from a token (never a written colour), and in both themes its words (`foreground`) pass 4.5:1 on the badge's fill (`card`) while the colour of its edge and icon passes 3:1 against that fill and against every surface a stack chip or the shared strip stands on, the way tokens.test.ts holds the rest of the palette.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { contrastRatio, parseOklch, toSrgb } from '@/test-utils/color'
import type { Mapping, Rgb } from '@/test-utils/color'
import { blockAfter } from '@/test-utils/cssBlocks'
import { MARK_FX } from './model'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, '../../../index.css'), 'utf8')
const marks = readFileSync(join(import.meta.dirname, 'fx.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')

type Theme = 'light' | 'dark'

function declarations(block: string): Map<string, string> {
  const found = new Map<string, string>()
  for (const match of block.matchAll(/(?:^|[\s;])--([a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    found.set(match[1] ?? '', (match[2] ?? '').trim())
  }
  return found
}

const root = declarations(blockAfter(css, '\n:root {'))
const dark = declarations(blockAfter(css, '\n.dark {'))
const VARS: Record<Theme, ReadonlyMap<string, string>> = { light: root, dark: new Map([...root, ...dark]) }
const MAPPINGS: readonly Mapping[] = ['clip', 'chroma']
const SURFACES = ['background', 'surface', 'table', 'raised', 'card'] as const

function valueOf(vars: ReadonlyMap<string, string>, name: string): string {
  const value = vars.get(name)
  if (value === undefined) throw new Error(`--${name} is not defined`)
  const alias = /^var\(--([a-z0-9-]+)\)$/.exec(value)
  return alias === null ? value : valueOf(vars, alias[1] ?? '')
}

const paint = (theme: Theme, token: string, mapping: Mapping): Rgb =>
  toSrgb(parseOklch(valueOf(VARS[theme], token)), mapping)

/** The token a mark's colour is set to in `fx.css`, by kind; the rule without a kind is the default. */
function tokenOf(kind: string): string {
  const rule = new RegExp(
    `\\.fx-mark\\[data-fx-mark='[^}]*${kind}'[^{]*\\{\\s*--mark:\\s*var\\(--([a-z-]+)\\)`,
  )
  const found = rule.exec(marks)?.[1]
  if (found !== undefined) return found
  throw new Error(`no colour for ${kind}`)
}

describe('the colours of the marks', () => {
  it('sets every kind from a token, and no colour is written in the badge', () => {
    for (const kind of MARK_FX) expect(tokenOf(kind)).toMatch(/^[a-z-]+$/)
    const badge = /\.fx-mark \{[^}]*\}/.exec(marks)?.[0] ?? ''
    expect(badge).not.toMatch(/oklch\(|rgb|#[0-9a-f]{3}/)
    expect(badge).toMatch(/background:\s*var\(--card\)/)
    expect(badge).toMatch(/color:\s*var\(--foreground\)/)
  })

  it('names each kind with an icon and a word, not a colour alone', () => {
    const source = readFileSync(join(import.meta.dirname, 'FxMark.tsx'), 'utf8')
    for (const kind of MARK_FX) expect(source).toMatch(new RegExp(`\\b${kind}: [A-Z]`))
  })
})

describe.each(['light', 'dark'] as const)('contrast in the %s theme', (theme) => {
  it('keeps the words at 4.5:1 on the badge', () => {
    for (const mapping of MAPPINGS) {
      expect(
        contrastRatio(paint(theme, 'foreground', mapping), paint(theme, 'card', mapping)),
      ).toBeGreaterThanOrEqual(4.5)
    }
  })

  it.each(MARK_FX.flatMap((kind) => SURFACES.map((surface) => ({ kind, surface }))))(
    'keeps the edge and icon of $kind at 3:1 on $surface',
    ({ kind, surface }) => {
      for (const mapping of MAPPINGS) {
        const ratio = contrastRatio(paint(theme, tokenOf(kind), mapping), paint(theme, surface, mapping))
        expect(ratio, `${tokenOf(kind)} on ${surface}, ${mapping}`).toBeGreaterThanOrEqual(3)
      }
    },
  )
})
