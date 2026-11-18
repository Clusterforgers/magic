// Goal: the lobby of a tournament match offers its seat 0 no host controls (visibility, the invite, Remove, Close game, other seats' teams), only what every player has, while an ordinary game's seat 0 keeps them.
import { screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { clearFlash } from '@/lib/flash'
import type { Teams } from '@/protocol/generated'
import { openLobby } from '@/test-utils/lobbyControls'

afterEach(clearFlash)

const NAMES = ['Ana', 'Bo', 'Cy', 'Di']
const ALTERNATE: Teams = { mode: 'teams', seats: [0, 1, 0, 1] }

const slot = (name: string): Element | null => document.querySelector(`[data-slot="${name}"]`)

describe('seat 0', () => {
  it.each<[string, boolean]>([
    ['an ordinary game', false],
    ['a tournament match', true],
  ])('of %s gets the host controls only when it hosts', async (_, tournament) => {
    await openLobby({ seat: 0, names: NAMES, teams: ALTERNATE, tournament })
    const hosting = !tournament
    await waitFor(() => expect(slot('lobby-visibility')).not.toBeNull())
    expect(screen.queryByRole('group', { name: 'Who can find this game' }) !== null).toBe(hosting)
    expect(slot('join-invite') !== null).toBe(hosting)
    expect(screen.queryAllByRole('button', { name: /^Remove/ }).length > 0).toBe(hosting)
    expect(screen.queryByRole('button', { name: 'Close game' }) !== null).toBe(hosting)
    expect(screen.queryByRole('button', { name: 'Leave game' }) !== null).toBe(!hosting)
    expect(slot('seat-host') !== null).toBe(hosting)
    // The host moves every seat; a player only their own.
    expect(document.querySelectorAll('[data-team-select]')).toHaveLength(hosting ? NAMES.length : 1)
  })
})
