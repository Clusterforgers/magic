// Goal: the session sends the auto-pass setting as one flat AutoPass message, follows the server's word in every Welcome (so a restarted server's default shows at once), changes its own state only when the message went out, and a repeated setting is still just a message.
import { describe, expect, it } from 'vitest'
import { startSession } from '@/test-utils/harness'
import { welcome } from '@/test-utils/socketSetup'

describe('the setting', () => {
  it('starts on, as the server does', () => {
    const h = startSession()
    expect(h.state().autoPass).toBe(true)
  })

  it.each([true, false])('is taken from the Welcome (%s)', (enabled) => {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    h.deliver({ ...welcome, auto_pass: enabled })
    expect(h.state().autoPass).toBe(enabled)
  })

  it('follows the Welcome of a later connection, whatever the page held before', () => {
    const h = startSession()
    h.join()
    expect(h.session.setAutoPass(false)).toBe(true)
    expect(h.state().autoPass).toBe(false)

    // The connection drops and the server comes back having forgotten it (a restart): its word is the default again.
    h.net.last.fail()
    h.timers.advance(250)
    h.open()
    h.deliver({ ...welcome, auto_pass: true })
    expect(h.state().autoPass).toBe(true)
  })
})

describe('setAutoPass', () => {
  it.each([true, false])('sends AutoPass { enabled: %s } and shows it', (enabled) => {
    const h = startSession()
    h.join()
    h.deliver({ ...welcome, auto_pass: !enabled })
    expect(h.session.setAutoPass(enabled)).toBe(true)
    expect(h.sentOf('AutoPass')).toEqual([{ type: 'AutoPass', enabled }])
    expect(h.state().autoPass).toBe(enabled)
  })

  it('sends nothing and changes nothing while the socket is not open', () => {
    const h = startSession()
    expect(h.session.setAutoPass(false)).toBe(false)
    expect(h.state().autoPass).toBe(true)
    h.join()
    h.net.last.fail()
    expect(h.session.setAutoPass(false)).toBe(false)
    expect(h.state().autoPass).toBe(true)
  })

  it('may be repeated: every call is a message', () => {
    const h = startSession()
    h.join()
    h.session.setAutoPass(false)
    h.session.setAutoPass(false)
    h.session.setAutoPass(true)
    expect(h.sentOf('AutoPass')).toEqual([
      { type: 'AutoPass', enabled: false },
      { type: 'AutoPass', enabled: false },
      { type: 'AutoPass', enabled: true },
    ])
    expect(h.state().autoPass).toBe(true)
  })

  it('is not part of the join: a seat that never touches it sends nothing about it', () => {
    const h = startSession()
    h.join()
    expect(h.sentTypes()).toEqual(['Join'])
  })
})
