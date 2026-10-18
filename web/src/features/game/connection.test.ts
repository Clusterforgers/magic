// Goal: the socket's status and failed attempts map to exactly one connection kind, and "cannot reach" begins at the fifth failure and no sooner.
import { describe, expect, it } from 'vitest'
import type { ConnectionStatus } from '@/net/socket'
import { connectionOf, UNREACHABLE_AFTER } from './connection'

const read = (status: ConnectionStatus, attempt = 0, notice: string | null = null) =>
  connectionOf({ status, attempt, notice })

describe('connectionOf', () => {
  it.each<[ConnectionStatus, string]>([
    ['idle', 'connecting'],
    ['connecting', 'connecting'],
    ['open', 'open'],
  ])('reads %s as %s', (status, kind) => {
    expect(read(status).kind).toBe(kind)
  })

  it.each(Array.from({ length: UNREACHABLE_AFTER - 1 }, (_, i) => i + 1))(
    'is still reconnecting after %i failed attempts, carrying the number',
    (attempt) => {
      expect(read('reconnecting', attempt)).toEqual({ kind: 'reconnecting', attempt })
    },
  )

  it.each([UNREACHABLE_AFTER, UNREACHABLE_AFTER + 1, 40])(
    'cannot reach the game after %i failed attempts',
    (attempt) => {
      expect(read('reconnecting', attempt)).toEqual({ kind: 'unreachable', attempt })
    },
  )

  it('is five attempts, as the screen says', () => {
    expect(UNREACHABLE_AFTER).toBe(5)
  })

  it.each<['replaced' | 'stopped']>([['replaced'], ['stopped']])(
    'carries the reason a seat was %s',
    (status) => {
      expect(read(status, 0, 'because')).toEqual({ kind: status, reason: 'because' })
      expect(read(status)).toEqual({ kind: status, reason: null })
    },
  )

  it('ignores failed attempts when the socket is not retrying', () => {
    expect(read('open', 9).kind).toBe('open')
    expect(read('connecting', 9).kind).toBe('connecting')
  })
})
