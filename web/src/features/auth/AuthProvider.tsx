import { useEffect } from 'react'
import type { ReactNode } from 'react'
import { AuthContext, appAuth } from './context'
import type { AuthStore } from './store'

/** Gives every page the sign-in state and asks the server, once, who is signed in. */
export function AuthProvider({ store = appAuth, children }: { store?: AuthStore; children: ReactNode }) {
  useEffect(() => {
    void store.getState().start()
  }, [store])
  return <AuthContext value={store}>{children}</AuthContext>
}
