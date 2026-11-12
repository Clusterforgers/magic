// Goal: the production build keeps every page's code in its own chunk and each chunk within its gzip budget: the entry every page loads, the table's chunk (the game page's, or the table's own when the replay page shares it), and every other chunk, so a heavy import that lands in the wrong place fails here.
import { execFile } from 'node:child_process'
import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'
import { gzipSync } from 'node:zlib'
import { afterAll, describe, expect, it } from 'vitest'

const run = promisify(execFile)
const webRoot = join(import.meta.dirname, '..')
const vite = join(webRoot, 'node_modules/vite/bin/vite.js')
let out = ''

/** The gzip size in kB of every script of a fresh production build, by file name without its hash. */
async function chunks(): Promise<Map<string, number>> {
  out = await mkdtemp(join(tmpdir(), 'bundle-budget-'))
  await run(process.execPath, [vite, 'build', '--outDir', out, '--emptyOutDir', '--logLevel', 'error'], {
    cwd: webRoot,
    env: { ...process.env, NODE_ENV: 'production' },
  })
  const sizes = new Map<string, number>()
  for (const name of await readdir(join(out, 'assets'))) {
    if (!name.endsWith('.js')) continue
    const code = await readFile(join(out, 'assets', name))
    sizes.set(name.replace(/-[\w-]{8}\.js$/, ''), gzipSync(code).length / 1024)
  }
  return sizes
}

/** The gzip budget in kB of a chunk, about a tenth above its size when it was set. */
function budgetOf(chunk: string): number {
  if (chunk === 'index') return 135
  // The table is shared by the game and replay pages, so it may stand in a chunk of its own under the table's budget.
  if (chunk === 'game._gameId' || chunk === 'GameTable') return 250
  return 52
}

afterAll(async () => {
  if (out !== '') await rm(out, { recursive: true, force: true })
})

describe('the production build', () => {
  const built = chunks()

  it('splits the code per page: the decks, builder, play and table pages each have a chunk of their own', async () => {
    const names = [...(await built).keys()]
    for (const page of ['decks', 'decks_._deckId', 'new', 'game._gameId']) expect(names).toContain(page)
  }, 90_000)

  it('keeps each chunk within its gzip budget', async () => {
    const over = [...(await built)]
      .filter(([name, size]) => size > budgetOf(name))
      .map(([name, size]) => `${name}: ${size.toFixed(1)} kB > ${budgetOf(name)} kB`)
    expect(over).toEqual([])
  }, 90_000)
})
