// Goal: the design tokens of index.css keep the contrast the design system promises in the light and the dark theme (4.5:1 for text, 3:1 for edges, rings and other non-text parts), including the six support marks of the deck check as text, edge and tinted pill, both themes define every colour, and the type, radius, elevation, motion and width scales hold their documented values, all computed from the stylesheet itself.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { contrastRatio, over, parseOklch, toSrgb } from '@/test-utils/color'
import type { Mapping, Rgb } from '@/test-utils/color'
import { blockAfter } from '@/test-utils/cssBlocks'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, 'index.css'), 'utf8')

type Theme = 'light' | 'dark'
type Vars = ReadonlyMap<string, string>

function declarations(block: string): Map<string, string> {
  const found = new Map<string, string>()
  for (const match of block.matchAll(/(?:^|[\s;])--([a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    found.set(match[1] ?? '', (match[2] ?? '').trim())
  }
  return found
}

// The heads include the line start: the first ".dark" of the file is the `@custom-variant`, not the block.
const rootBlock = blockAfter(css, '\n:root {')
const darkBlock = blockAfter(css, '\n.dark {')
const root = declarations(rootBlock)
const dark = declarations(darkBlock)
const theme = declarations(blockAfter(css, '@theme inline'))

const VARS: Record<Theme, Vars> = { light: root, dark: new Map([...root, ...dark]) }
const MAPPINGS: readonly Mapping[] = ['clip', 'chroma']

function valueOf(vars: Vars, name: string): string {
  const value = vars.get(name)
  if (value === undefined) throw new Error(`--${name} is not defined`)
  const alias = /^var\(--([a-z0-9-]+)\)$/.exec(value)
  return alias === null ? value : valueOf(vars, alias[1] ?? '')
}

/** The sRGB colour of a token in a theme; a token with alpha has no single colour, so it throws. */
function paint(themeName: Theme, name: string, mapping: Mapping): Rgb {
  const color = parseOklch(valueOf(VARS[themeName], name))
  if (color.alpha !== 1) throw new Error(`--${name} is translucent`)
  return toSrgb(color, mapping)
}

const SURFACES = ['background', 'surface', 'table', 'raised'] as const
const FILLS = [...SURFACES, 'muted', 'secondary', 'accent'] as const
const STATUS = ['success', 'warning', 'destructive'] as const
const MANA = ['white', 'blue', 'black', 'red', 'green', 'gold', 'colorless'] as const
const SUPPORT = ['verified', 'read', 'partial', 'manual', 'stale', 'unplayable'] as const

const TEXT_MIN = 4.5
const EDGE_MIN = 3

interface Pair {
  /** What the pair is for, in words. */
  use: string
  fg: string
  bg: string
  min: number
}

const pairs: Pair[] = []
const text = (use: string, fg: string, bgs: readonly string[]): void => {
  for (const bg of bgs) pairs.push({ use: `${use} text`, fg, bg, min: TEXT_MIN })
}
const edge = (use: string, fg: string, bgs: readonly string[]): void => {
  for (const bg of bgs) pairs.push({ use: `${use} edge`, fg, bg, min: EDGE_MIN })
}

text('body', 'foreground', FILLS)
text('secondary', 'muted-foreground', FILLS)
text('card', 'card-foreground', ['card'])
text('menu', 'popover-foreground', ['popover'])
text('secondary button', 'secondary-foreground', ['secondary'])
text('highlighted item', 'accent-foreground', ['accent'])
text('link', 'primary', SURFACES)
text('main action', 'on-primary', ['primary'])
text('tooltip', 'background', ['foreground'])
text('your-turn pill', 'on-turn-you', ['turn-you'])
text('opponent turn', 'turn-them', SURFACES)
text('state', 'success', SURFACES)
text('state', 'warning', SURFACES)
text('state', 'destructive', SURFACES)
for (const name of MANA) text(`${name} mana symbol`, `on-mana-${name}`, [`mana-${name}`])
for (const name of SUPPORT) text(`${name} support mark`, `support-${name}`, SURFACES)

edge('field and control', 'border-strong', SURFACES)
edge('field', 'input', SURFACES)
edge('focus ring', 'ring', FILLS)
edge('main action', 'primary', SURFACES)
edge('active seat', 'turn-you-edge', SURFACES)
edge('opponent turn', 'turn-them', SURFACES)
edge('state', 'success', SURFACES)
edge('state', 'warning', SURFACES)
edge('state', 'destructive', SURFACES)
for (const name of SUPPORT) edge(`${name} support mark`, `support-${name}`, FILLS)

describe.each(['light', 'dark'] as const)('contrast in the %s theme', (themeName) => {
  it.each(pairs)('$use: $fg on $bg is at least $min:1', ({ fg, bg, min }) => {
    for (const mapping of MAPPINGS) {
      const ratio = contrastRatio(paint(themeName, fg, mapping), paint(themeName, bg, mapping))
      expect(ratio, `${fg} on ${bg}, ${mapping}`).toBeGreaterThanOrEqual(min)
    }
  })

  // A hovered main action and the tinted states paint a share of their colour over the ground. Tints sit on the
  // page, panels and menus; the table keeps flat colours, because a tint over its darker grey costs the contrast.
  const GROUNDS = ['background', 'surface', 'raised'] as const
  const TINTS: ReadonlyArray<{ use: string; fg: string; of: string; share: number }> = [
    { use: 'main action hovered', fg: 'on-primary', of: 'primary', share: 0.9 },
    ...STATUS.map((name) => ({ use: `${name} tint`, fg: name, of: name, share: 0.12 })),
    { use: 'destructive tint hovered', fg: 'destructive', of: 'destructive', share: 0.15 },
  ]
  // A support mark also sits on the highlighted (hovered) row of the deck pane.
  const SUPPORT_TINTS = SUPPORT.map((name) => ({
    use: `${name} support mark`,
    fg: `support-${name}`,
    of: `support-${name}`,
    share: 0.12,
  }))
  const cases = [
    ...TINTS.flatMap((tint) => GROUNDS.map((ground) => ({ ...tint, ground }))),
    ...SUPPORT_TINTS.flatMap((tint) => [...GROUNDS, 'accent'].map((ground) => ({ ...tint, ground }))),
  ]
  it.each(cases)('$use: $fg on $of at $share over $ground is at least 4.5:1', ({ fg, of, share, ground }) => {
    for (const mapping of MAPPINGS) {
      const tinted = over(paint(themeName, of, mapping), paint(themeName, ground, mapping), share)
      expect(
        contrastRatio(paint(themeName, fg, mapping), tinted),
        `${fg} on ${of} at ${share}`,
      ).toBeGreaterThanOrEqual(TEXT_MIN)
    }
  })

  it('stacks the surfaces: the table sunken, then the page, the panel and the menu on top', () => {
    const lightness = (name: string): number => parseOklch(valueOf(VARS[themeName], name)).l
    expect(lightness('table')).toBeLessThan(lightness('background'))
    expect(lightness('background')).toBeLessThan(lightness('surface'))
    // The dark theme lifts a menu by lightening it; the light theme keeps it as white as a panel.
    expect(lightness('raised')).toBeGreaterThanOrEqual(lightness('surface'))
  })
})

describe('the contrast maths', () => {
  const white = toSrgb(parseOklch('oklch(1 0 0)'), 'clip')
  const black = toSrgb(parseOklch('oklch(0 0 0)'), 'clip')

  it('gives 21:1 for black on white and 1:1 for a colour on itself', () => {
    expect(contrastRatio(black, white)).toBeCloseTo(21, 5)
    expect(contrastRatio(white, white)).toBe(1)
  })

  it('matches a published pair: #767676 on white is 4.54:1', () => {
    const grey = [0x76 / 255, 0x76 / 255, 0x76 / 255] as const
    expect(contrastRatio(grey, white)).toBeCloseTo(4.54, 2)
  })

  it('reads mid grey in oklch as the sRGB grey a browser paints', () => {
    const [red, green, blue] = toSrgb(parseOklch('oklch(0.6 0 0)'), 'clip').map((channel) =>
      Math.round(channel * 255),
    )
    expect([red, green, blue]).toEqual([red, red, red])
    expect(red).toBe(0x80)
  })

  it('brings a colour outside the gamut in, by cutting it or by lowering its chroma', () => {
    const wild = parseOklch('oklch(0.74 0.14 275)')
    for (const mapping of MAPPINGS) {
      for (const channel of toSrgb(wild, mapping)) {
        expect(channel).toBeGreaterThanOrEqual(0)
        expect(channel).toBeLessThanOrEqual(1)
      }
    }
  })

  it('blends a colour over another by its share', () => {
    expect(over([1, 0, 0], [0, 0, 1], 0.25)).toEqual([0.25, 0, 0.75])
  })

  it.each(['red', 'oklch(0.5 0.1)', 'oklch(a b c)'])('refuses %s', (value) => {
    expect(() => parseOklch(value)).toThrow()
  })
})

describe('the colour roles', () => {
  const ROLES = [
    'background',
    'surface',
    'table',
    'raised',
    'foreground',
    'muted-foreground',
    'border',
    'border-strong',
    'ring',
    'primary',
    'on-primary',
    'turn-you',
    'on-turn-you',
    'turn-you-edge',
    'turn-them',
    'success',
    'warning',
    'destructive',
  ]

  it.each(['light', 'dark'] as const)('has every role of the design system in the %s theme', (themeName) => {
    for (const role of ROLES) expect(parseOklch(valueOf(VARS[themeName], role)).alpha, role).toBe(1)
  })

  it('keeps the names shadcn components read', () => {
    for (const name of [
      'card',
      'card-foreground',
      'popover',
      'popover-foreground',
      'primary-foreground',
      'secondary',
      'secondary-foreground',
      'muted',
      'accent',
      'accent-foreground',
      'input',
      'radius',
    ]) {
      expect(root.has(name), name).toBe(true)
    }
  })

  it('gives the dark theme its own value for every colour except the mana symbols, which look the same in both', () => {
    const colours = [...root].filter(([, value]) => value.startsWith('oklch('))
    for (const [name] of colours) {
      if (name.startsWith('mana-') || name.startsWith('on-mana-')) continue
      expect(dark.has(name), `.dark lacks --${name}`).toBe(true)
    }
  })

  it('maps every colour variable to a Tailwind colour, so each has a utility', () => {
    const colours = new Set([...root.keys(), ...dark.keys()])
    colours.delete('radius')
    for (const name of [...colours].filter((name) => name.startsWith('duration-'))) colours.delete(name)
    for (const name of colours) expect(theme.get(`color-${name}`), `--color-${name}`).toBe(`var(--${name})`)
  })

  it('draws the mana symbols with the same fill and ink in both themes', () => {
    for (const name of MANA) {
      expect(dark.has(`mana-${name}`)).toBe(false)
      expect(dark.has(`on-mana-${name}`)).toBe(false)
    }
  })
})

describe('the scales', () => {
  const px = (value: string | undefined): number => {
    const match = /^([\d.]+)(rem|px)$/.exec(value ?? '')
    if (match === null) throw new Error(`not a length: ${value}`)
    return Number(match[1]) * (match[2] === 'rem' ? 16 : 1)
  }

  it.each([
    ['label', 11, 16],
    ['xs', 12, 16],
    ['sm', 14, 20],
    ['base', 16, 24],
    ['lg', 18, 28],
    ['2xl', 24, 32],
    ['4xl', 36, 40],
  ])('sets the %s size to %i px on a %i px line', (name, size, line) => {
    expect(px(theme.get(`text-${name}`))).toBe(size)
    expect(px(theme.get(`text-${name}--line-height`))).toBe(line)
  })

  it('scales the display size with the width, from 32 to 48 px', () => {
    expect(theme.get('text-display')).toBe('clamp(2rem, 4vw, 3rem)')
  })

  it('draws on a 4 px grid', () => {
    expect(px(theme.get('spacing'))).toBe(4)
  })

  it('rounds a card by 6 px, a control by 8 and a panel by 12', () => {
    const radius = px(root.get('radius'))
    const factor = (name: string): number =>
      Number(/\* ([\d.]+)\)/.exec(theme.get(`radius-${name}`) ?? '')?.[1] ?? 1)
    expect(radius * factor('md')).toBe(6)
    expect(theme.get('radius-lg')).toBe('var(--radius)')
    expect(radius).toBe(8)
    expect(radius * factor('xl')).toBe(12)
  })

  it('lifts menus and dialogs with the three shadows of the design system', () => {
    expect(theme.get('shadow-e1')).toBe('0 1px 2px oklch(0 0 0 / 0.08)')
    expect(theme.get('shadow-e2')).toBe('0 4px 12px oklch(0 0 0 / 0.12)')
    expect(theme.get('shadow-e3')).toBe('0 12px 32px oklch(0 0 0 / 0.2)')
  })

  it('moves in 100, 200 and 300 ms, easing out on the way in and in on the way out', () => {
    expect([root.get('duration-fast'), root.get('duration-base'), root.get('duration-slow')]).toEqual([
      '100ms',
      '200ms',
      '300ms',
    ])
    expect(theme.get('ease-out')).toBe('cubic-bezier(0.2, 0, 0, 1)')
    expect(theme.get('ease-in')).toBe('cubic-bezier(0.3, 0, 1, 1)')
  })

  it('ends the largest breakpoint at 1600 px and gives pages three widths', () => {
    expect(px(theme.get('breakpoint-2xl'))).toBe(1600)
    expect([theme.get('container-narrow'), theme.get('container-page'), theme.get('container-wide')]).toEqual(
      ['28rem', '48rem', '72rem'],
    )
  })

  it('declares the colour scheme of each theme, so scrollbars and native controls follow it', () => {
    expect(rootBlock).toMatch(/color-scheme:\s*light;/)
    expect(darkBlock).toMatch(/color-scheme:\s*dark;/)
  })
})

describe('the shared motion utility', () => {
  const popup = blockAfter(css, '@utility popup-motion')
  const motion = blockAfter(popup, '@media (prefers-reduced-motion: no-preference)')
  const still = popup.replace(motion, '')

  it('fades for everyone and moves only where motion is welcome', () => {
    expect(still).toMatch(/opacity:\s*0/)
    expect(still).not.toMatch(/transform/)
    expect(motion).toMatch(/transform:\s*scale/)
  })
})
