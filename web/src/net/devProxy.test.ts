// Goal: the Vite config sends /api and /ws to the game server on port 8080, upgrades websockets, and leaves the Host header alone so the server's Origin check passes.
import { describe, expect, it } from 'vitest'
import config from '../../vite.config'

const proxy = config.server?.proxy ?? {}

describe('the Vite dev proxy', () => {
  it('proxies exactly /api and /ws', () => {
    expect(Object.keys(proxy).sort()).toEqual(['/api', '/ws'])
  })

  it.each(['/api', '/ws'])('sends %s to the game server', (path) => {
    expect(proxy[path]).toMatchObject({ target: 'http://127.0.0.1:8080' })
  })

  it('upgrades websockets on /ws and not on /api', () => {
    expect(proxy['/ws']).toMatchObject({ ws: true })
    expect(proxy['/api']).not.toMatchObject({ ws: true })
  })

  it.each(['/api', '/ws'])('keeps the original Host header on %s (changeOrigin off)', (path) => {
    const rule = proxy[path]
    expect(typeof rule).toBe('object')
    expect(rule).not.toMatchObject({ changeOrigin: true })
  })

  it('does not rewrite the paths: the server serves /api/... and /ws/{game}/{token} itself', () => {
    for (const rule of Object.values(proxy)) expect(rule).not.toHaveProperty('rewrite')
  })
})
