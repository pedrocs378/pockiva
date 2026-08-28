export type PairingConfig = {
  token: string
  socketUrl: string
}

export type PairingResult = { status: 'ready'; config: PairingConfig } | { status: 'missing-token' | 'invalid-url' }

export const parsePairingUrl = (url: URL): PairingResult => {
  const token = url.searchParams.get('token')?.trim()
  if (!token) return { status: 'missing-token' }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') return { status: 'invalid-url' }

  const socketUrl = new URL('/controller', url)
  socketUrl.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  socketUrl.search = ''
  socketUrl.hash = ''

  return { status: 'ready', config: { token, socketUrl: socketUrl.toString() } }
}
