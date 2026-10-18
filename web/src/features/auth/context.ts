import { createContext, useContext } from 'react'
import { useStore } from 'zustand'
import { createAuthStore, offlineAccounts } from './store'
import type { AuthState, AuthStore } from './store'

/** The store of the running app: `main.tsx` provides it, and `AuthProvider` asks the server who is signed in. */
export const appAuth: AuthStore = createAuthStore()

// A page rendered without a provider (most tests) has no accounts: it behaves as it did before they existed.
const withoutAccounts: AuthStore = createAuthStore({
  api: offlineAccounts,
  initial: { status: 'unavailable' },
})

/** Provided by `AuthProvider`; read it through `useAuth`. */
export const AuthContext = createContext<AuthStore>(withoutAccounts)

/** The auth store of the page, for code that calls actions (`useAuthStore().getState().login(...)`). */
export function useAuthStore(): AuthStore {
  return useContext(AuthContext)
}

/** Reads a slice of the sign-in state; re-renders only when the slice changes. Pass a stable selector. */
export function useAuth<T>(selector: (state: AuthState) => T): T {
  return useStore(useAuthStore(), selector)
}
