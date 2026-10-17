// Goal: the socket joins first and once, builds the address from the page origin, survives garbage frames and fatal errors, and stays idle after disconnect.
import { describe, expect, it } from 'vitest'
import { GAME, TOKEN, join, nextWait, open, setup, welcome } from '@/test-utils/socketSetup'
import type { Setup } from '@/test-utils/socketSetup'
import { socketUrl } from './socket'

describe('socketUrl', () => {
  it.each([
    ['http://localhost:5173', 'g', 't', 'ws://localhost:5173/ws/g/t'],
    ['https://play.example', 'g', 't', 'wss://play.example/ws/g/t'],
    ['http://127.0.0.1:8080', GAME, 'a-b_c', `ws://127.0.0.1:8080/ws/${GAME}/a-b_c`],
    ['https://play.example:8443', 'g', 't', 'wss://play.example:8443/ws/g/t'],
    ['http://localhost:5173', 'a/b', 'x y?', 'ws://localhost:5173/ws/a%2Fb/x%20y%3F'],
  ])('%s with game %s and token %s is %s', (origin, game, token, expected) => {
    expect(socketUrl(game, token, origin)).toBe(expected)
  })

  it('connects to the address on the page origin', () => {
    const { socket, net } = setup({ origin: 'https://play.example' })
    socket.connect()
    expect(net.last.url).toBe(`wss://play.example/ws/${GAME}/${TOKEN}`)
  })
})

describe('joining', () => {
  it('sends Join as the first frame, once, whatever is sent after it', () => {
    const t = setup()
    t.socket.connect()
    expect(t.net.last.frames).toEqual([])
    expect(t.socket.send({ type: 'Ready' })).toBe(false)
    t.net.last.open()
    expect(t.socket.send({ type: 'Ready' })).toBe(true)
    t.net.last.receive(welcome)
    t.socket.send({ type: 'Resync' })
    expect(t.net.last.sent).toEqual([join, { type: 'Ready' }, { type: 'Resync' }])
  })

  it('sends one Join per connection, first, on every reconnect', () => {
    const t = setup()
    open(t)
    for (let attempt = 0; attempt < 4; attempt += 1) {
      t.net.last.serverClose(1001)
      t.timers.advance(nextWait(t))
      t.net.last.open()
      t.socket.send({ type: 'Ready' })
      t.net.last.receive(welcome)
    }
    expect(t.net.sockets).toHaveLength(5)
    for (const socket of t.net.sockets.slice(1)) expect(socket.sentTypes).toEqual(['Join', 'Ready'])
    expect(t.net.sockets[0]?.sentTypes).toEqual(['Join'])
  })

  it('reports connecting, the Welcome, then open, in that order', () => {
    const t = setup()
    open(t)
    expect(
      t.events.map((event) =>
        event.type === 'message' ? event.message.type : event.type === 'status' ? event.status : event.type,
      ),
    ).toEqual(['connecting', 'Welcome', 'open'])
    expect(t.socket.status).toBe('open')
  })

  it('does not write a second Join if the socket reports open twice', () => {
    const t = setup()
    t.socket.connect()
    t.net.last.open()
    t.net.last.open()
    expect(t.net.last.sentTypes).toEqual(['Join'])
  })
})

describe('connect and disconnect', () => {
  it.each<[string, (t: Setup) => void]>([
    ['connecting', (t) => t.socket.connect()],
    ['open', (t) => open(t)],
    [
      'reconnecting',
      (t) => {
        open(t)
        t.net.last.fail()
      },
    ],
  ])('connect() while %s changes nothing', (_state, arrange) => {
    const t = setup()
    arrange(t)
    const sockets = t.net.sockets.length
    const events = t.events.length
    const timers = t.timers.pending()
    t.socket.connect()
    t.socket.connect()
    expect(t.net.sockets).toHaveLength(sockets)
    expect(t.events).toHaveLength(events)
    expect(t.timers.pending()).toEqual(timers)
  })

  it('survives the connect, disconnect, connect of a double-run effect', () => {
    const t = setup()
    t.socket.connect()
    t.socket.disconnect()
    t.socket.connect()
    expect(t.net.sockets).toHaveLength(2)
    const [first, second] = t.net.sockets
    expect(first?.closedByClient).toEqual({ code: 1000, reason: 'client closing' })
    // The abandoned socket's late events must not touch the live one.
    first?.open()
    first?.receive(welcome)
    first?.serverClose(1000)
    expect(first?.frames).toEqual([])
    expect(t.statuses()).toEqual(['connecting', 'idle', 'connecting'])
    second?.open()
    second?.receive(welcome)
    expect(second?.sentTypes).toEqual(['Join'])
    expect(t.socket.status).toBe('open')
  })

  it.each<[string, (t: Setup) => void]>([
    ['connecting', (t) => t.socket.connect()],
    ['open', (t) => open(t)],
    [
      'waiting to reconnect',
      (t) => {
        open(t)
        t.net.last.fail()
      },
    ],
  ])(
    'disconnect() while %s closes with 1000, cancels the retry and ignores what comes later',
    (_state, arrange) => {
      const t = setup()
      arrange(t)
      const live = t.net.last
      t.socket.disconnect()
      expect(t.socket.status).toBe('idle')
      expect(t.timers.pending()).toEqual([])
      expect(t.lastStatus()).toMatchObject({ status: 'idle', reason: null })
      t.timers.advance(60_000)
      live.serverClose(1006)
      live.receive(welcome)
      expect(t.net.sockets.length).toBeGreaterThan(0)
      expect(t.socket.status).toBe('idle')
      const count = t.events.length
      t.socket.disconnect()
      expect(t.events).toHaveLength(count)
      expect(t.socket.send({ type: 'Ready' })).toBe(false)
    },
  )

  it('closes a socket that is open with code 1000 and a reason', () => {
    const t = setup()
    open(t)
    t.socket.disconnect()
    expect(t.net.last.closedByClient).toEqual({ code: 1000, reason: 'client closing' })
  })

  it.each<[string, number]>([
    ['stopped', 1008],
    ['replaced', 4000],
  ])('connect() after the socket %s starts again from attempt 0', (_what, code) => {
    const t = setup()
    open(t)
    t.net.last.serverClose(code)
    t.socket.connect()
    expect(t.net.sockets).toHaveLength(2)
    expect(t.lastStatus()).toMatchObject({ status: 'connecting', attempt: 0, reason: null })
  })

  it('stops with a reason when the browser refuses to make the socket', () => {
    const t = setup({
      createSocket: () => {
        throw new SyntaxError('bad url')
      },
    })
    t.socket.connect()
    expect(t.lastStatus()).toMatchObject({ status: 'stopped', reason: expect.stringContaining('refused') })
    expect(t.timers.pending()).toEqual([])
  })
})

describe('frames', () => {
  it('turns garbage into frame errors and keeps the connection and later messages', () => {
    const t = setup()
    open(t)
    t.net.last.receive('{oops')
    t.net.last.receive({ type: 'Nope' })
    t.net.last.receive({ type: 'Chat', from: 'me', text: 5 })
    t.net.last.receiveRaw(new ArrayBuffer(4))
    t.net.last.receive({ type: 'Chat', from: 1, text: 'still here' })
    const kinds = t.events.flatMap((event) => (event.type === 'frame_error' ? [event.error.kind] : []))
    expect(kinds).toEqual(['not_json', 'unknown_type', 'bad_shape', 'not_text'])
    expect(t.events.at(-1)).toEqual({
      type: 'message',
      message: { type: 'Chat', from: 1, text: 'still here' },
    })
    expect(t.socket.status).toBe('open')
    expect(t.timers.pending()).toEqual([])
  })

  it.each([
    ['UnsupportedVersion', /protocol versions/],
    ['UnknownGame', /does not exist/],
  ])('an Error{%s} stops the socket for good with a readable reason', (code, reason) => {
    const t = setup()
    t.socket.connect()
    t.net.last.open()
    t.net.last.receive({ type: 'Error', code, message: 'detail' })
    expect(t.events.map((event) => event.type)).toEqual(['status', 'message', 'status'])
    expect(t.lastStatus()).toMatchObject({ status: 'stopped', reason: expect.stringMatching(reason) })
    expect(t.net.last.closedByClient).toEqual({ code: 1000, reason: 'client closing' })
    // The server follows its Error with a close; neither that nor time may revive the socket.
    const count = t.events.length
    t.net.last.serverClose(1002)
    t.timers.advance(120_000)
    expect(t.events).toHaveLength(count)
    expect(t.net.sockets).toHaveLength(1)
    expect(t.timers.pending()).toEqual([])
  })

  it.each(['BadMessage', 'StaleDecision', 'IllegalAnswer', 'Internal'])(
    'an Error{%s} is passed on and leaves the connection alone',
    (code) => {
      const t = setup()
      open(t)
      t.net.last.receive({ type: 'Error', code, message: 'detail' })
      expect(t.events.at(-1)).toEqual({
        type: 'message',
        message: { type: 'Error', code, message: 'detail' },
      })
      expect(t.socket.status).toBe('open')
    },
  )

  it('ignores what a replaced socket still delivers', () => {
    const t = setup()
    open(t)
    const old = t.net.last
    old.fail()
    t.timers.advance(250)
    t.net.last.open()
    t.net.last.receive(welcome)
    const count = t.events.length
    old.receive({ type: 'Chat', from: 1, text: 'from the past' })
    old.serverClose(1006)
    expect(t.events).toHaveLength(count)
    expect(t.socket.status).toBe('open')
  })
})

describe('retire', () => {
  it.each([1001, 1006, 1013])('makes a lost connection (code %i) final', (code) => {
    const t = setup()
    open(t)
    t.socket.retire()
    t.net.last.serverClose(code)
    expect(t.lastStatus()).toMatchObject({ status: 'stopped', reason: null })
    expect(t.timers.pending()).toEqual([])
  })

  it('is forgotten when the socket is connected again', () => {
    const t = setup()
    open(t)
    t.socket.retire()
    t.net.last.serverClose(1006)
    t.socket.connect()
    t.net.last.open()
    t.net.last.serverClose(1006)
    expect(t.socket.status).toBe('reconnecting')
  })
})
