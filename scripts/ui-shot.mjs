#!/usr/bin/env node
// Screenshots a page with headless Firefox driven over WebDriver BiDi. Firefox's own `--screenshot` fires at `load`,
// before a SPA has rendered, so it gives a blank page; this waits for a condition first.
// Usage: node scripts/ui-shot.mjs <url> <out-prefix> [--widths 1400,390] [--height 900] [--wait "<js condition>"] [--full] [--click "<css>"]
// Writes <out-prefix>-<width>.png per width and prints whether the page overflows horizontally.
import { spawn, execSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

function options(argv) {
  const [url, prefix, ...rest] = argv
  const flags = { widths: '1400,390', height: '900', wait: 'document.body && document.body.innerText.length > 0', full: false, click: '' }
  for (let at = 0; at < rest.length; at += 1) {
    const name = rest[at].replace(/^--/, '')
    if (name === 'full') flags.full = true
    else if (name in flags) flags[name] = rest[(at += 1)] ?? ''
    else throw new Error(`unknown option ${rest[at]}`)
  }
  if (!url || !prefix) throw new Error('usage: ui-shot.mjs <url> <out-prefix> [--widths 1400,390] [--height 900] [--wait <js>] [--full] [--click <css>]')
  return { url, prefix, widths: flags.widths.split(',').map(Number), height: Number(flags.height), wait: flags.wait, full: flags.full, click: flags.click }
}

function firefoxPath() {
  if (process.env.FIREFOX) return process.env.FIREFOX
  return execSync('ls -d /nix/store/*-firefox-devedition-*/bin/firefox-devedition | head -n 1').toString().trim()
}

async function connect(port) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      return await new Promise((resolve, reject) => {
        const socket = new WebSocket(`ws://127.0.0.1:${port}/session`)
        socket.onopen = () => resolve(socket)
        socket.onerror = () => reject(new Error('no socket'))
      })
    } catch {
      await sleep(200)
    }
  }
  throw new Error('could not connect to Firefox over WebDriver BiDi')
}

async function launch(port) {
  const profile = mkdtempSync(join(tmpdir(), 'ui-shot-'))
  const child = spawn(firefoxPath(), ['--headless', '--no-remote', '--profile', profile, `--remote-debugging-port=${port}`, 'about:blank'], { stdio: 'ignore' })
  const socket = await connect(port)
  let nextId = 1
  const waiting = new Map()
  socket.onmessage = (message) => {
    const data = JSON.parse(message.data)
    const pending = data.id === undefined ? undefined : waiting.get(data.id)
    if (!pending) return
    waiting.delete(data.id)
    if (data.type === 'error') pending.reject(new Error(`${data.error}: ${data.message}`))
    else pending.resolve(data.result)
  }
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = nextId++
      waiting.set(id, { resolve, reject })
      socket.send(JSON.stringify({ id, method, params }))
    })
  await send('session.new', { capabilities: {} })
  const context = (await send('browsingContext.getTree', {})).contexts[0].context
  const evaluate = async (expression) => {
    const result = await send('script.evaluate', { expression, target: { context }, awaitPromise: true, resultOwnership: 'none' })
    if (result.type === 'exception') throw new Error(`script error: ${result.exceptionDetails.text}`)
    return result.result.value
  }
  return {
    open: (url) => send('browsingContext.navigate', { context, url, wait: 'complete' }),
    viewport: (width, height) => send('browsingContext.setViewport', { context, viewport: { width, height } }),
    evaluate,
    async waitFor(condition, timeout = 15000) {
      const started = Date.now()
      while (Date.now() - started < timeout) {
        try {
          if (await evaluate(`Boolean(${condition})`)) return
        } catch {
          // The page may still be loading.
        }
        await sleep(150)
      }
      throw new Error(`timed out waiting for: ${condition}`)
    },
    async shoot(file, full) {
      const result = await send('browsingContext.captureScreenshot', { context, origin: full ? 'document' : 'viewport' })
      writeFileSync(file, Buffer.from(result.data, 'base64'))
    },
    async close() {
      socket.close()
      child.kill('SIGKILL')
      await sleep(200)
      rmSync(profile, { recursive: true, force: true })
    },
  }
}

const config = options(process.argv.slice(2))
const browser = await launch(9333 + (process.pid % 500))
try {
  await browser.viewport(config.widths[0], config.height)
  await browser.open(config.url)
  await browser.waitFor(config.wait)
  if (config.click) {
    await browser.evaluate(`document.querySelector(${JSON.stringify(config.click)}).click()`)
    await sleep(300)
  }
  for (const width of config.widths) {
    await browser.viewport(width, config.height)
    await sleep(400)
    const file = `${config.prefix}-${width}.png`
    await browser.shoot(file, config.full)
    const sizes = JSON.parse(await browser.evaluate('JSON.stringify({scroll: document.documentElement.scrollWidth, client: document.documentElement.clientWidth})'))
    console.log(`${file}${sizes.scroll > sizes.client ? ` OVERFLOWS horizontally (${sizes.scroll} > ${sizes.client})` : ''}`)
  }
} finally {
  await browser.close()
}
