import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import type { AccountRow } from '@/protocol/generated/AccountRow'
import { AccountsPage } from '../AccountsPage'

/**
 * `/dev/accounts?many=1&names=long`: the real Accounts page signed in as the admin over an in-memory list, for
 * looking at it. `many=1` lists thirty friends; `names=long` gives each a username of 32 characters. Making an
 * account adds a row; a new password and Remove work on the list as if a server answered them.
 */
export function DevAccountsRoute() {
  const { many, names } = useSearch({ from: '/dev/accounts' })
  const auth = useMemo(() => {
    const rows = fixtureRows(many === true ? 30 : 4, names === 'long')
    return createAuthStore({
      api: {
        ...offlineAccounts,
        myGames: async () => [],
        listAccounts: async () => [...rows],
        createAccount: async ({ username }) => {
          const made = row(rows.length, username)
          rows.push(made)
          return { id: made.id, username }
        },
        resetPassword: async () => undefined,
        removeAccount: async (id) => {
          const at = rows.findIndex((account) => account.id === id)
          if (at >= 0) rows.splice(at, 1)
        },
      },
      initial: {
        status: 'signed-in',
        user: { id: rows[0]?.id ?? 'dev-0', username: 'Admin', is_admin: true },
      },
    })
  }, [many, names])
  return (
    <AuthContext value={auth}>
      <AccountsPage />
    </AuthContext>
  )
}

function row(index: number, username: string): AccountRow {
  const day = String((index % 28) + 1).padStart(2, '0')
  return {
    id: `00000000-0000-4000-8000-${String(index).padStart(12, '0')}`,
    username,
    created_at: `2026-10-${day}T09:30:00Z`,
    is_admin: index === 0,
  }
}

function fixtureRows(count: number, long: boolean): AccountRow[] {
  return Array.from({ length: count }, (_, index) => {
    const name = index === 0 ? 'Admin' : long ? `friend_${index}_`.padEnd(32, 'x') : `friend_${index}`
    return row(index, name)
  })
}
