// Goal: every button and card squashes to 0.97 in the frame it is pressed and springs back only where motion is welcome, a menu trigger or a disabled control never squashes, the primary pass button swells once when it comes to the viewer, and under a coarse pointer every button, the switch, the stack chips and the lands chip hit at 44 px through a pseudo-element that never moves the layout.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Button, buttonVariants } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { blockAfter } from '@/test-utils/cssBlocks'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const read = (path: string): string =>
  readFileSync(join(import.meta.dirname, path), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
const css = read('feel.css')
const app = read('index.css')
const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'
const base = blockAfter(css, '@layer base')
const press = blockAfter(css, '@utility press')
const hit = blockAfter(css, '@utility hit-area')
const variable = (name: string): string => new RegExp(`${name}:\\s*([^;]+);`).exec(css)?.[1]?.trim() ?? ''
const classesOf = (value: string): string[] => value.split(/\s+/).filter(Boolean)

describe('the press', () => {
  it('squashes to 0.97 and springs back past 1', () => {
    expect(variable('--press-scale')).toBe('0.97')
    expect(variable('--ease-spring')).toMatch(/cubic-bezier\(0\.34, 1\.56/)
    expect(Number.parseFloat(variable('--press-in-ms'))).toBeLessThanOrEqual(100)
    expect(Number.parseFloat(variable('--press-out-ms'))).toBeLessThanOrEqual(300)
  })

  it('moves only where motion is welcome, for every button and the utility alike', () => {
    for (const block of [base, press]) {
      const motion = blockAfter(block, NO_PREFERENCE)
      expect(motion).toMatch(
        /:active:not\(\[aria-haspopup\], :disabled, \[aria-disabled='true'\]\)\s*\{\s*scale: var\(--press-scale\)/,
      )
      expect(block.replace(motion, '')).not.toMatch(/scale/)
    }
    expect(blockAfter(base, NO_PREFERENCE)).toMatch(/:where\(button, a\[data-slot='button'\]\)/)
  })

  it('animates scale only, never a property that lays the page out again', () => {
    const transitions = [...css.matchAll(/transition(?:-property)?:\s*([^;]+);/g)].map(
      (match) => match[1] ?? '',
    )
    for (const list of transitions) expect(list).not.toMatch(/\b(width|height|top|left|margin|padding)\b/)
  })

  it('is part of the app stylesheet', () => {
    expect(app).toMatch(/@import '\.\/feel\.css';/)
  })

  it('is on every button, which keeps the colour transitions it replaced', () => {
    const classes = classesOf(buttonVariants())
    expect(classes).toContain('press')
    expect(classes).not.toContain('transition-colors')
    expect(press).toMatch(/background-color/)
  })
})

describe('the primary pass button', () => {
  const ready = css.slice(css.lastIndexOf(NO_PREFERENCE))
  const motion = blockAfter(ready, NO_PREFERENCE)

  it('swells once when it comes to the viewer, only where motion is welcome', () => {
    expect(motion).toMatch(
      /\[data-slot='primary-action'\]:not\(:disabled\)\s*\{\s*animation: feel-ready var\(--ready-ms\) var\(--ease-out\);/,
    )
    expect(blockAfter(motion, '@keyframes feel-ready')).toMatch(/scale: 1\.06/)
    expect(Number.parseFloat(variable('--ready-ms'))).toBeLessThanOrEqual(450)
    expect(css.replace(motion, '')).not.toMatch(/feel-ready \w/)
  })

  it('keeps no fill, so a press squashes it at once afterwards', () => {
    expect(motion).not.toMatch(/forwards|both/)
  })
})

describe('the hit area', () => {
  it('is 44 px under a coarse pointer, as a pseudo-element centred on the control', () => {
    expect(variable('--hit-min')).toBe('44px')
    const coarse = blockAfter(hit, '@media (pointer: coarse)')
    expect(coarse).toMatch(/&::after/)
    expect(coarse).toMatch(/position: absolute/)
    expect(coarse).toMatch(/inset-block: min\(0px, calc\(\(100% - var\(--hit-min\)\) \/ 2\)\)/)
    expect(coarse).toMatch(/inset-inline: min\(0px, calc\(\(100% - var\(--hit-min\)\) \/ 2\)\)/)
    expect(hit.replace(coarse, '')).not.toMatch(/inset|width|height|padding/)
  })

  it.each(['xs', 'sm', 'icon-xs', 'icon-sm', 'default', 'lg'] as const)(
    'is on a %s button, positioned',
    (size) => {
      render(<Button size={size}>Go</Button>)
      const button = screen.getByRole('button', { name: 'Go' })
      expect(button).toHaveClass('hit-area', 'relative')
    },
  )

  it('gives way to a button placed on its own', () => {
    render(<Button className="absolute">Go</Button>)
    const button = screen.getByRole('button', { name: 'Go' })
    expect(button).toHaveClass('absolute')
    expect(button).not.toHaveClass('relative')
  })

  it('makes the 20 px switch 44 px high under a coarse pointer', () => {
    render(<Switch aria-label="Sounds" />)
    // 20 px track plus 12 px above and below.
    expect(screen.getByRole('switch', { name: 'Sounds' })).toHaveClass(
      'relative',
      'pointer-coarse:after:-inset-y-3',
    )
  })

  it.each([
    ['features/table/StackChips.tsx', 2],
    ['features/table/LandSummary.tsx', 1],
  ])('is on the small table chips of %s', (path, count) => {
    const source = readFileSync(join(import.meta.dirname, path), 'utf8')
    expect(source.match(/relative hit-area/g)?.length).toBe(count)
  })
})
