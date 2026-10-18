// Goal: the production build contains nothing of the dev table fixture (its marker text and card names are in no output file), while a build with DEV on does contain it, so the check could fail.
import { execFile } from 'node:child_process'
import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'
import { afterAll, describe, expect, it } from 'vitest'
import { COMMANDERS } from './dev/cards'
import { FIXTURE_MARKER } from './dev/fixture'

const run = promisify(execFile)
const webRoot = join(import.meta.dirname, '../../..')
const vite = join(webRoot, 'node_modules/vite/bin/vite.js')
const made: string[] = []

/** Builds the app into a fresh directory with the given NODE_ENV and returns the text of every file in it. */
async function build(nodeEnv: 'production' | 'development'): Promise<Map<string, string>> {
  const out = await mkdtemp(join(tmpdir(), `table-${nodeEnv}-`))
  made.push(out)
  await run(process.execPath, [vite, 'build', '--outDir', out, '--emptyOutDir', '--logLevel', 'error'], {
    cwd: webRoot,
    env: { ...process.env, NODE_ENV: nodeEnv },
  })
  const files = new Map<string, string>()
  for (const entry of await readdir(out, { recursive: true, withFileTypes: true })) {
    if (!entry.isFile() || !/\.(js|css|html)$/.test(entry.name)) continue
    const path = join(entry.parentPath, entry.name)
    files.set(path.slice(out.length), await readFile(path, 'utf8'))
  }
  return files
}

const FIXTURE_TRACES = [FIXTURE_MARKER, COMMANDERS[0].name, 'Sakashima of a Thousand Faces']
const filesWith = (files: Map<string, string>, text: string): string[] =>
  [...files].filter(([, content]) => content.includes(text)).map(([name]) => name)

afterAll(async () => {
  await Promise.all(made.map((dir) => rm(dir, { recursive: true, force: true })))
})

describe('the dev table route in a build', () => {
  // Both builds run at once; each takes a second or two.
  const builds = Promise.all([build('production'), build('development')])

  it('leaves the fixture out of the production bundle', async () => {
    const [production] = await builds
    expect(production.size).toBeGreaterThan(3)
    for (const trace of FIXTURE_TRACES) expect(filesWith(production, trace), trace).toEqual([])
  }, 90_000)

  it('still serves the route, which answers "not found" in production', async () => {
    const [production] = await builds
    const route = [...production].filter(([name]) => /dev\.table/.test(name))
    expect(route).toHaveLength(1)
  }, 90_000)

  it('puts the fixture in a chunk of its own when DEV is on, which is how the check above can fail', async () => {
    const [, development] = await builds
    const [name] = filesWith(development, FIXTURE_MARKER)
    expect(name).toMatch(/DevTableRoute/)
    expect(filesWith(development, COMMANDERS[0].name).length).toBeGreaterThan(0)
  }, 90_000)
})
