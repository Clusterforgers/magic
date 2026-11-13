// Goal: no source or style file of the web client keeps a merge conflict marker, which the token tests would read past.
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { expect, it } from 'vitest'

const SRC = import.meta.dirname

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return files(path)
    return /\.(?:tsx?|css)$/.test(name) ? [path] : []
  })
}

it('keeps no conflict markers', () => {
  const marked = files(SRC).filter((path) => /^(?:<{7} |>{7} |={7}$)/m.test(readFileSync(path, 'utf8')))
  expect(marked.map((path) => relative(SRC, path))).toEqual([])
})
