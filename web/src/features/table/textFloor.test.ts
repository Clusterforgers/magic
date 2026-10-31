// Goal: no source of the table sets a text size under the 12 px floor of the type scale (0.75rem), so the layout gate (`e2e/layout.spec.ts`) finds no small text there: the parser reads every way a size is written (`text-[...]` in px, rem, em and percent, `text-[length:clamp(...)]`, the named steps of the type scale in `index.css` such as `text-label`, and `font-size` in a style or a stylesheet) and the sources of the table keep every one at the floor or above.
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'

const TABLE = import.meta.dirname
const FLOOR_PX = 12

/** The named steps of the type scale, in px: `--text-label: 0.6875rem` in `index.css` makes `text-label` 11 px. */
function typeScale(css: string): ReadonlyMap<string, number> {
  const scale = new Map<string, number>()
  for (const [, name, value, unit] of css.matchAll(/--text-([a-z0-9]+):\s*(\d*\.?\d+)(rem|px);/g))
    scale.set(name ?? '', toPx(Number(value), unit ?? 'px'))
  return scale
}

const toPx = (value: number, unit: string): number => (unit === 'rem' ? value * 16 : value)
const round = (px: number): number => Math.round(px * 100) / 100

/**
 * The sizes in px that a line of source writes. A size relative to the parent (`em`, `%`) below 1 shrinks text that
 * already sits at the floor, so it counts as the floor times its factor; the other relative sizes never shrink.
 */
function writtenSizes(line: string, scale: ReadonlyMap<string, number>): number[] {
  const sizes: number[] = []
  for (const [, value, unit] of line.matchAll(/\btext-\[(\d*\.?\d+)(rem|px)\]/g))
    sizes.push(toPx(Number(value), unit ?? 'px'))
  // `text-[length:clamp(4px,calc(var(--card-w)*0.045),7px)]`: the smallest size it can come to is the one that counts.
  for (const [, expression] of line.matchAll(/\btext-\[length:([^\]]+)\]/g)) {
    const found = [...(expression ?? '').matchAll(/(\d*\.?\d+)(rem|px)\b/g)].map(([, value, unit]) =>
      toPx(Number(value), unit ?? 'px'),
    )
    if (found.length > 0) sizes.push(Math.min(...found))
  }
  for (const [, value] of line.matchAll(/\btext-\[(\d*\.?\d+)em\]/g))
    if (Number(value) < 1) sizes.push(round(Number(value) * FLOOR_PX))
  for (const [, value] of line.matchAll(/\btext-\[(\d*\.?\d+)%\]/g))
    if (Number(value) < 100) sizes.push(round((Number(value) / 100) * FLOOR_PX))
  for (const [, step] of line.matchAll(/\btext-([a-z][a-z0-9]*)\b(?![-[])/g)) {
    const px = scale.get(step ?? '')
    if (px !== undefined) sizes.push(px)
  }
  for (const [, value, unit] of line.matchAll(/font-?[sS]ize['"]?\s*[:=]\s*['"]?(\d*\.?\d+)(rem|px)\b/g))
    sizes.push(toPx(Number(value), unit ?? 'px'))
  return sizes
}

function sourcesIn(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return sourcesIn(path)
    return /\.(?:tsx?|css)$/.test(name) && !/\.test\.tsx?$/.test(name) ? [path] : []
  })
}

const SCALE = typeScale(readFileSync(join(TABLE, '..', '..', 'index.css'), 'utf8'))

describe('the type scale', () => {
  it.each([
    ['label', 11],
    ['xs', 12],
    ['sm', 14],
    ['base', 16],
  ])('reads text-%s as %d px', (step, px) => {
    expect(SCALE.get(step)).toBe(px)
  })
})

describe('the sizes a source writes', () => {
  it.each<[string, number[]]>([
    ['className="text-[0.6rem] font-bold"', [9.6]],
    ['className="text-[0.65rem] leading-4"', [10.4]],
    ['className="text-[0.6875rem]"', [11]],
    ['className="text-[11px] sm:text-[10px]"', [11, 10]],
    ['className="size-4 text-[1rem]"', [16]],
    // A size relative to its parent counts when it shrinks the text, not when it keeps or grows it.
    ['className="text-[0.62em]"', [7.44]],
    ['className="text-[0.85em] font-semibold"', [10.2]],
    ['className="text-[85%]"', [10.2]],
    ['className="text-[1em] text-[120%]"', []],
    // The smallest size a clamp can come to.
    ['className="size-4 text-[length:clamp(4px,calc(var(--card-w)*0.045),7px)]"', [4]],
    ['className="text-[length:clamp(0.5rem,2vw,1rem)]"', [8]],
    ['className="text-[length:var(--size)]"', []],
    // The named steps of the scale.
    ['className="text-label font-medium"', [11]],
    ['className="md:text-label"', [11]],
    ['className="text-sm text-label"', [14, 11]],
    ['className="text-xs text-foreground text-center"', [12]],
    ['className="text-label-foreground"', []],
    // A size written as a style or in a stylesheet.
    ['  font-size: 0.5rem;', [8]],
    ["style={{ fontSize: '10px' }}", [10]],
    ['  font-size: 1.25rem;', [20]],
  ])('reads %s as %j px', (line, sizes) => {
    expect(writtenSizes(line, SCALE)).toEqual(sizes)
  })
})

describe('text sizes in the table', () => {
  const files = sourcesIn(TABLE)

  it('has files to check', () => {
    expect(files.length).toBeGreaterThan(40)
  })

  it('keeps every written size at the 12 px floor or above', () => {
    const small: string[] = []
    for (const file of files) {
      readFileSync(file, 'utf8')
        .split('\n')
        .forEach((line, index) => {
          // A comment may quote a size it advises against.
          if (/^\s*(?:\/\/|\*|\/\*)/.test(line)) return
          if (writtenSizes(line, SCALE).some((px) => px < FLOOR_PX - 0.001))
            small.push(`${relative(TABLE, file)}:${index + 1}: ${line.trim().slice(0, 100)}`)
        })
    }
    expect(small).toEqual([])
  })
})
