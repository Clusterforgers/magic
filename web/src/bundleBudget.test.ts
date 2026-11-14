// Goal: the production build keeps every page's code in its own chunk and each chunk within its gzip budget (the entry every page loads, React and the router in a vendor chunk, the table's chunk shared by the game, watch and replay pages, and every other chunk, so a heavy import that lands in the wrong place fails here), preloads the font and writes a gzip twin of every text file.
import { execFile } from 'node:child_process'
import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'
import { gunzipSync, gzipSync } from 'node:zlib'
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
  if (chunk === 'index') return 70
  // React and the router, which every page loads.
  if (chunk === 'vendor') return 105
  // The game, watch and replay pages share the table, in the chunk of `GamePage` or a chunk of its own.
  if (chunk === 'GamePage' || chunk === 'GameTable') return 250
  return 52
}

afterAll(async () => {
  if (out !== '') await rm(out, { recursive: true, force: true })
})

describe('the production build', () => {
  const built = chunks()

  it('splits the code per page: the decks, builder, play, game and watch pages each have a chunk of their own, the table one they share', async () => {
    const names = [...(await built).keys()]
    for (const page of [
      'vendor',
      'decks',
      'decks_._deckId',
      'new',
      'game._gameId',
      'watch._gameId',
      'GamePage',
    ])
      expect(names).toContain(page)
  }, 90_000)

  it('preloads the Latin font from the page, and the font it names is in the build', async () => {
    await built
    const page = await readFile(join(out, 'index.html'), 'utf8')
    const href =
      /<link rel="preload" href="\/(assets\/geist-latin-wght-normal-[\w-]+\.woff2)" as="font"/.exec(page)?.[1]
    expect(href).toBeDefined()
    const assets = await readdir(join(out, 'assets'))
    expect(assets.map((name) => `assets/${name}`)).toContain(href)
  }, 90_000)

  it('writes a gzip twin of every script, stylesheet and the page that unpacks to the same bytes, for nginx to send as it is', async () => {
    await built
    const files = [
      'index.html',
      ...(await readdir(join(out, 'assets')))
        .filter((name) => /\.(js|css)$/.test(name))
        .map((name) => join('assets', name)),
    ]
    for (const file of files) {
      const twin = gunzipSync(await readFile(join(out, `${file}.gz`)))
      expect(twin.equals(await readFile(join(out, file))), file).toBe(true)
    }
  }, 90_000)

  it('keeps each chunk within its gzip budget', async () => {
    const over = [...(await built)]
      .filter(([name, size]) => size > budgetOf(name))
      .map(([name, size]) => `${name}: ${size.toFixed(1)} kB > ${budgetOf(name)} kB`)
    expect(over).toEqual([])
  }, 90_000)
})
