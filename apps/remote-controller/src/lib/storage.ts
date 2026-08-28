export type StorageBackend = Pick<globalThis.Storage, 'getItem' | 'setItem'>

export class Storage {
  constructor(private readonly backend: StorageBackend) {}

  read(key: string): unknown {
    const value = this.backend.getItem(key)
    if (value === null) return null

    try {
      return JSON.parse(value)
    } catch {
      return value
    }
  }

  write(key: string, value: unknown): void {
    this.backend.setItem(key, JSON.stringify(value))
  }
}

export const browserStorage = new Storage({
  getItem: (key) => window.localStorage.getItem(key),
  setItem: (key, value) => window.localStorage.setItem(key, value)
})
