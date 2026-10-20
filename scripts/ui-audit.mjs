#!/usr/bin/env node
// Audits pages of a running dev server against the layout rules of docs/ui-design.md in Chromium, the browser the e2e
// uses: no sideways scroll, nothing cut off, no text under 11 px, every action given is in view and not covered, and,
// with --fit, no scroll down. It loads the TypeScript helper of web/e2e/layout.spec.ts as it is, so it needs Node 22.18+.
// Usage: node scripts/ui-audit.mjs <url or /path>... [--base http://127.0.0.1:5173] [--sizes 1280x720,390x844]
//          [--themes light,dark] [--fit] [--action "<css>"]... [--exempt "<css>"]... [--wait "<js condition>"] [--problems]
// --action names a primary action to find in view and uncovered; --exempt names a region whose contents may run past
// the window on purpose (a strip that scrolls, the art of a card).
// Prints one row for every page, size and theme and exits 1 if any row has a problem. CHROMIUM_PATH picks the browser
// (on NixOS: CHROMIUM_PATH=$(ls -d /nix/store/*-chromium-[0-9]*/bin/chromium | head -n 1)).
import { execSync } from 'node:child_process'
import { createRequire } from 'node:module'
import {
  formatTable,
  measure,
  parseSizes,
  SIZES,
  settle,
  sizeName,
  storeTheme,
  TEXT_FLOOR,
  THEMES,
} from '../web/e2e/layoutAudit.ts'

const USAGE =
  'usage: ui-audit.mjs <url or /path>... [--base <url>] [--sizes WxH,WxH] [--themes light,dark] [--fit] [--action <css>]... [--exempt <css>]... [--wait <js>] [--problems]'

function options(argv) {
  const config = {
    pages: [],
    base: 'http://127.0.0.1:5173',
    sizes: SIZES,
    themes: [...THEMES],
    fit: false,
    actions: [],
    exempt: [],
    wait: "document.querySelector('main') !== null && document.body.innerText.trim().length > 0",
    problemsOnly: false,
  }
  for (let at = 0; at < argv.length; at += 1) {
    const arg = argv[at]
    const value = () => {
      at += 1
      if (argv[at] === undefined) throw new Error(`${arg} needs a value\n${USAGE}`)
      return argv[at]
    }
    if (arg === '--base') config.base = value()
    else if (arg === '--sizes') config.sizes = parseSizes(value())
    else if (arg === '--themes') config.themes = value().split(',').map((name) => name.trim())
    else if (arg === '--fit') config.fit = true
    else if (arg === '--action') config.actions.push(value())
    else if (arg === '--exempt') config.exempt.push(value())
    else if (arg === '--wait') config.wait = value()
    else if (arg === '--problems') config.problemsOnly = true
    else if (arg.startsWith('--')) throw new Error(`unknown option ${arg}\n${USAGE}`)
    else config.pages.push(arg)
  }
  if (config.pages.length === 0) throw new Error(USAGE)
  for (const theme of config.themes) {
    if (!THEMES.includes(theme)) throw new Error(`unknown theme "${theme}" (light or dark)`)
  }
  return config
}

function chromiumPath() {
  if (process.env.CHROMIUM_PATH) return process.env.CHROMIUM_PATH
  try {
    return execSync('ls -d /nix/store/*-chromium-[0-9]*/bin/chromium | head -n 1', { stdio: ['ignore', 'pipe', 'ignore'] })
      .toString()
      .trim()
  } catch {
    return undefined
  }
}

const frames = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))

const config = options(process.argv.slice(2))
// The browser comes from the web client's own dependencies, so this script has no package of its own.
const { chromium } = createRequire(new URL('../web/package.json', import.meta.url))('@playwright/test')
const browser = await chromium.launch({ executablePath: chromiumPath() || undefined })
const rows = []
try {
  const ask = {
    actions: Object.fromEntries(config.actions.map((css) => [css, css])),
    reveal: !config.fit,
    exempt: config.exempt,
    floor: TEXT_FLOOR,
  }
  for (const theme of config.themes) {
    const context = await browser.newContext({ colorScheme: theme, viewport: config.sizes[0] })
    await context.addInitScript(storeTheme, theme)
    const page = await context.newPage()
    for (const target of config.pages) {
      const url = target.startsWith('/') ? new URL(target, config.base).href : target
      const opened = await page.goto(url).then(
        () => page.waitForFunction(config.wait, undefined, { timeout: 15_000 }).then(() => null),
        (error) => error,
      )
      for (const size of config.sizes) {
        if (opened !== null) {
          rows.push({ page: target, size: sizeName(size), theme, problems: [`did not open: ${String(opened.message).split('\n')[0]}`] })
          continue
        }
        await page.setViewportSize(size)
        const problems = await settle(
          async () => {
            await page.evaluate(frames)
            return page.evaluate(measure, ask)
          },
          { fits: config.fit },
        )
        rows.push({ page: target, size: sizeName(size), theme, problems })
      }
    }
    await context.close()
  }
} finally {
  await browser.close()
}
const shown = config.problemsOnly ? rows.filter((row) => row.problems.length > 0) : rows
if (shown.length > 0) console.log(formatTable(shown))
const failed = rows.filter((row) => row.problems.length > 0).length
console.log(`${rows.length - failed} of ${rows.length} checks ok`)
process.exit(failed === 0 ? 0 : 1)
