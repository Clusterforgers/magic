// Goal: no class in the table sets a text size under the 12 px floor of the type scale (0.75rem), so the layout gate (`e2e/layout.spec.ts`) finds no small text there: the parser reads rem and px sizes of `text-[...]` and the sources of the table keep every one at the floor or above.
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'

const TABLE = import.meta.dirname
const FLOOR_PX = 12

/** The sizes in px that a line of source writes as `text-[0.6rem]` or `text-[10px]`; `em` sizes follow their parent. */
function writtenSizes(line: string): number[] {
  return [...line.matchAll(/\btext-\[(\d*\.?\d+)(rem|px)\]/g)].map(([, value, unit]) =>
    unit === 'rem' ? Number(value) * 16 : Number(value),
  )
}

function sourcesIn(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return sourcesIn(path)
    return /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) ? [path] : []
  })
}

describe('the sizes a class writes', () => {
  it.each([
    ['className="text-[0.6rem] font-bold"', [9.6]],
    ['className="text-[0.65rem] leading-4"', [10.4]],
    ['className="text-[0.6875rem]"', [11]],
    ['className="text-[11px] sm:text-[10px]"', [11, 10]],
    ['className="text-[0.62em]"', []],
    ['className="text-sm text-label"', []],
    ['className="size-4 text-[1rem]"', [16]],
  ])('reads %s as %j px', (line, sizes) => {
    expect(writtenSizes(line)).toEqual(sizes)
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
          if (writtenSizes(line).some((px) => px < FLOOR_PX - 0.001))
            small.push(`${relative(TABLE, file)}:${index + 1}: ${line.trim().slice(0, 100)}`)
        })
    }
    expect(small).toEqual([])
  })
})
