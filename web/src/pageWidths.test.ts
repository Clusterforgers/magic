// Goal: every page takes its width from the scale of PageShell (narrow, page, wide): no PageShell sets a max width of its own, and none names a width the scale does not have.
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'
import { PAGE_WIDTHS } from '@/components/pageWidths'

const SRC = import.meta.dirname

function sourcesIn(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return sourcesIn(path)
    return /\.tsx$/.test(name) && !/\.test\.tsx$/.test(name) ? [path] : []
  })
}

/** The opening tag of every `<PageShell …>` in the file, with the line it starts on. */
function shells(file: string): Array<{ at: string; tag: string }> {
  const text = readFileSync(file, 'utf8')
  return [...text.matchAll(/<PageShell\b[^>]*>/g)].map((match) => ({
    at: `${relative(SRC, file)}:${text.slice(0, match.index).split('\n').length}`,
    tag: match[0],
  }))
}

const TAGS = sourcesIn(SRC).flatMap(shells)

describe('page widths', () => {
  it('finds the pages that sit in a PageShell', () => {
    expect(TAGS.length).toBeGreaterThan(20)
  })

  it('has no PageShell that picks a max width of its own', () => {
    const found = TAGS.filter(({ tag }) => /(?<![\w-])max-w-/.test(tag)).map(({ at }) => at)
    expect(found).toEqual([])
  })

  it('names only widths of the scale', () => {
    const found = TAGS.flatMap(({ at, tag }) => {
      const width = /\bwidth="([^"]+)"/.exec(tag)?.[1]
      return width === undefined || (PAGE_WIDTHS as readonly string[]).includes(width)
        ? []
        : [`${at}: ${width}`]
    })
    expect(found).toEqual([])
  })
})
