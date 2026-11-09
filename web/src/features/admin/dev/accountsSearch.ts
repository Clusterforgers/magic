/** The address parameters of `/dev/accounts`, read leniently: whatever makes no sense is ignored. */
export interface DevAccountsSearch {
  /** `1` lists thirty accounts instead of four. */
  many?: true
  /** `long` gives every friend a username of 32 characters. */
  names?: 'long'
}

/** Reads `many` and `names` from the address. */
export function parseDevAccountsSearch(raw: Record<string, unknown>): DevAccountsSearch {
  const many = raw.many === 1 || raw.many === '1' || raw.many === true
  return {
    ...(many ? { many: true as const } : {}),
    ...(raw.names === 'long' ? { names: 'long' as const } : {}),
  }
}
