import { getRouteApi } from '@tanstack/react-router'
import { AuthScreen } from './AuthScreen'

const login = getRouteApi('/login')
const register = getRouteApi('/register')

/** The component of `/login`: the form, sending the visitor on to `?next=` afterwards. */
export function LoginRoute() {
  return <AuthScreen mode="login" next={login.useSearch().next} />
}

/** The component of `/register`. */
export function RegisterRoute() {
  return <AuthScreen mode="register" next={register.useSearch().next} />
}
