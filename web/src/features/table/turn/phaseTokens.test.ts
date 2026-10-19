// Goal: the four phase themes are whole and readable in both themes: each has an accent, a soft tint and the text on the accent, set for light and dark and mapped to utilities; the text on the accent and on the tint reaches 4.5:1; the accent reaches 3:1 against every surface, its tint and the washed table; text on the washed table keeps 4.5:1 and the wash stays at 6 % or less; and the four accents stay plainly apart, for full vision and under protanopia, deuteranopia and tritanopia.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { contrastRatio, over, parseOklch } from '@/test-utils/color'
import type { Mapping, Rgb } from '@/test-utils/color'
import { DARK, MAPPINGS, paint, ROOT, THEME, THEMES, VARS, valueOf } from '@/test-utils/themeTokens'
import type { Theme } from '@/test-utils/themeTokens'
import { colourDifference, DICHROMACIES, simulate } from '@/test-utils/vision'
import { PHASES } from './phase'
import type { PhaseId } from './phase'

const IDS = Object.keys(PHASES) as PhaseId[]
const SURFACES = ['background', 'surface', 'table', 'raised'] as const

/** The least CIEDE2000 difference between two accents: 10 is a plain difference between two colours side by side. */
const MIN_DIFFERENCE = 12

const phaseCss = readFileSync(join(import.meta.dirname, 'phase.css'), 'utf8')
const wash = /color-mix\(in srgb, var\(--phase\) (\d+(?:\.\d+)?)%, var\(--table\)\)/.exec(phaseCss)
/** The share of the accent the table's ground takes, as the stylesheet writes it. */
const WASH = Number(wash?.[1] ?? Number.NaN) / 100

const tokens = (id: PhaseId): [string, string, string] => [
  `phase-${id}`,
  `phase-${id}-soft`,
  `on-phase-${id}`,
]

describe('the tokens', () => {
  it.each(IDS)('has an accent, a soft tint and the text on the accent for %s, in both themes', (id) => {
    for (const token of tokens(id)) {
      expect(ROOT.has(token), `${token} in :root`).toBe(true)
      expect(DARK.has(token), `${token} in .dark`).toBe(true)
      expect(THEME.get(`color-${token}`), `--color-${token}`).toBe(`var(--${token})`)
      for (const name of THEMES) expect(parseOklch(valueOf(VARS[name], token)).alpha, token).toBe(1)
    }
  })

  it('has the theme in force as three live tokens, neutral until the table root names a phase', () => {
    for (const token of ['phase', 'phase-soft', 'on-phase']) {
      expect(ROOT.get(token), token).toMatch(/^var\(--[a-z-]+\)$/)
      expect(THEME.get(`color-${token}`), `--color-${token}`).toBe(`var(--${token})`)
    }
    expect(ROOT.get('phase')).toBe('var(--border)')
  })

  it.each(IDS)(
    'keeps the hue of %s in its tint and in the other theme, so one phase is one colour family',
    (id) => {
      const hue = (theme: Theme, token: string): number => parseOklch(valueOf(VARS[theme], token)).h
      const [accent, soft] = tokens(id)
      for (const theme of THEMES) expect(Math.abs(hue(theme, accent) - hue(theme, soft))).toBeLessThan(1)
      expect(Math.abs(hue('light', accent) - hue('dark', accent))).toBeLessThan(1)
    },
  )
})

describe('the wash', () => {
  it('is read from the stylesheet, at most 6 % of the accent over the table', () => {
    expect(Number.isFinite(WASH)).toBe(true)
    expect(WASH).toBeGreaterThan(0)
    expect(WASH).toBeLessThanOrEqual(0.06)
  })
})

interface Case {
  use: string
  /** The colour that has to be seen. */
  fg: (colours: Colours) => Rgb
  /** The colour it is seen on. */
  bg: (colours: Colours) => Rgb
  min: number
}

interface Colours {
  accent: Rgb
  soft: Rgb
  on: Rgb
  washed: Rgb
  foreground: Rgb
  muted: Rgb
  surface: (name: (typeof SURFACES)[number]) => Rgb
}

function coloursOf(theme: Theme, id: PhaseId, mapping: Mapping): Colours {
  const [accent, soft, on] = tokens(id).map((token) => paint(theme, token, mapping)) as [Rgb, Rgb, Rgb]
  return {
    accent,
    soft,
    on,
    washed: over(accent, paint(theme, 'table', mapping), WASH),
    foreground: paint(theme, 'foreground', mapping),
    muted: paint(theme, 'muted-foreground', mapping),
    surface: (name) => paint(theme, name, mapping),
  }
}

const cases: Case[] = [
  { use: 'text on the accent (your turn’s badge)', fg: (c) => c.on, bg: (c) => c.accent, min: 4.5 },
  {
    use: 'text on the soft tint (another seat’s badge)',
    fg: (c) => c.foreground,
    bg: (c) => c.soft,
    min: 4.5,
  },
  {
    use: 'accent on its soft tint (the edge of the tag and the current pip)',
    fg: (c) => c.accent,
    bg: (c) => c.soft,
    min: 3,
  },
  ...SURFACES.map((name) => ({
    use: `accent on the ${name} (the rule, the strip’s edge, the pips)`,
    fg: (c: Colours) => c.accent,
    bg: (c: Colours) => c.surface(name),
    min: 3,
  })),
  { use: 'accent on the washed table', fg: (c) => c.accent, bg: (c) => c.washed, min: 3 },
  { use: 'text on the washed table', fg: (c) => c.foreground, bg: (c) => c.washed, min: 4.5 },
  { use: 'secondary text on the washed table', fg: (c) => c.muted, bg: (c) => c.washed, min: 4.5 },
]

describe.each(THEMES)('contrast of the phase themes in the %s theme', (theme) => {
  const each = IDS.flatMap((id) => cases.map((entry) => ({ ...entry, id })))
  it.each(each)('$id: $use is at least $min:1', ({ id, fg, bg, min }) => {
    for (const mapping of MAPPINGS) {
      const colours = coloursOf(theme, id, mapping)
      expect(contrastRatio(fg(colours), bg(colours)), `${id}, ${mapping}`).toBeGreaterThanOrEqual(min)
    }
  })
})

describe.each(THEMES)('the four accents in the %s theme', (theme) => {
  const visions = ['full vision', ...DICHROMACIES] as const
  const pairs = IDS.flatMap((first, at) => IDS.slice(at + 1).map((second) => [first, second] as const))
  const accent = (id: PhaseId, vision: (typeof visions)[number], mapping: Mapping): Rgb => {
    const colour = paint(theme, `phase-${id}`, mapping)
    return vision === 'full vision' ? colour : simulate(colour, vision)
  }

  it('make six pairs, so none is left out', () => {
    expect(pairs).toHaveLength(6)
  })

  it.each(visions.flatMap((vision) => pairs.map(([first, second]) => ({ vision, first, second }))))(
    '$first and $second differ by at least 12 under $vision',
    ({ vision, first, second }) => {
      for (const mapping of MAPPINGS) {
        const apart = colourDifference(accent(first, vision, mapping), accent(second, vision, mapping))
        expect(apart, `${first} and ${second}, ${vision}, ${mapping}`).toBeGreaterThanOrEqual(MIN_DIFFERENCE)
      }
    },
  )

  it('are not the gold of the viewer’s own turn, for full vision or any dichromacy', () => {
    for (const vision of visions) {
      const gold = paint(theme, 'turn-you', 'clip')
      const seenGold = vision === 'full vision' ? gold : simulate(gold, vision)
      for (const id of IDS) {
        const apart = colourDifference(accent(id, vision, 'clip'), seenGold)
        expect(apart, `${id} and the your-turn gold, ${vision}`).toBeGreaterThanOrEqual(MIN_DIFFERENCE)
      }
    }
  })
})
