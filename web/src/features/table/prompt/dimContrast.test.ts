// Goal: a board choice dims the cards outside it to 0.4 but leaves their names readable: the rule never fades the whole card, only the parts of its face without the name, so a name keeps its own tokens' contrast, which passes WCAG AA (4.5:1) in both themes; faded as a whole over the table it would not.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { contrastRatio, over, parseOklch, toSrgb } from '@/test-utils/color'
import type { Rgb } from '@/test-utils/color'
import { blockAfter } from '@/test-utils/cssBlocks'

const index = readFileSync(join(import.meta.dirname, '../../../index.css'), 'utf8')
const band = readFileSync(join(import.meta.dirname, 'promptBand.css'), 'utf8')

function declarations(block: string): Map<string, string> {
  const found = new Map<string, string>()
  for (const match of block.matchAll(/(?:^|[\s;])--([a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    found.set(match[1] ?? '', (match[2] ?? '').trim())
  }
  return found
}

const root = declarations(blockAfter(index, '\n:root {'))
const dark = new Map([...root, ...declarations(blockAfter(index, '\n.dark {'))])

function paint(vars: ReadonlyMap<string, string>, name: string): Rgb {
  const value = vars.get(name)
  if (value === undefined) throw new Error(`--${name} is not defined`)
  const alias = /^var\(--([a-z0-9-]+)\)$/.exec(value)
  return alias === null ? toSrgb(parseOklch(value), 'clip') : paint(vars, alias[1] ?? '')
}

describe('the names of dimmed cards', () => {
  const rule = blockAfter(band, '[data-board-choice]\n')

  it('fade the parts of the face without the name, never the whole card', () => {
    expect(rule.split('&')[0]).not.toContain('opacity')
    expect(rule).toContain("[data-slot='card-face'] > :not(:has([data-name]))")
    expect(blockAfter(rule, ':not(:has([data-name])) {')).toMatch(/opacity:\s*0\.4/)
  })

  it.each([
    ['light', root],
    ['dark', dark],
  ] as const)('keep 4.5:1 or more in the %s theme, which a card faded whole would lose', (_theme, vars) => {
    const name = paint(vars, 'card-foreground')
    const face = paint(vars, 'card')
    expect(contrastRatio(name, face)).toBeGreaterThanOrEqual(4.5)
    const table = paint(vars, 'table')
    const faded = contrastRatio(over(name, table, 0.4), over(face, table, 0.4))
    expect(faded).toBeLessThan(contrastRatio(name, face))
  })
})
