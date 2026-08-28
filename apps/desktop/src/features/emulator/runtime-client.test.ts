import { beforeEach, describe, expect, it, vi } from 'vitest'

type MockChannel<T> = { onmessage: (message: T) => void }

const tauriMocks = vi.hoisted(() => {
  class Channel<T> {
    onmessage: (message: T) => void = () => undefined
  }

  return {
    Channel,
    channels: [] as Array<MockChannel<unknown>>,
    invoke: vi.fn(),
    open: vi.fn()
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  Channel: class<T> extends tauriMocks.Channel<T> {
    constructor() {
      super()
      tauriMocks.channels.push(this as MockChannel<unknown>)
    }
  },
  invoke: tauriMocks.invoke
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: tauriMocks.open }))

import { tauriEmulatorRuntimeClient as client } from './runtime-client'
import { FRAME_PACKET_BYTE_LENGTH } from './video'

const pausedSnapshot = {
  phase: 'paused',
  rom: {
    title: 'Fixture',
    fileName: 'fixture.gb',
    romIdentity: 'fixture',
    mapper: 'rom-only',
    compatibility: 'dmg'
  },
  error: null
}

const createFramePacket = () => {
  const buffer = new ArrayBuffer(FRAME_PACKET_BYTE_LENGTH)
  const view = new DataView(buffer)
  view.setBigUint64(0, 7n, true)
  view.setUint16(8, 160, true)
  view.setUint16(10, 144, true)
  return buffer
}

describe('tauri emulator runtime client', () => {
  beforeEach(() => {
    tauriMocks.invoke.mockReset()
    tauriMocks.open.mockReset()
    tauriMocks.channels.length = 0
  })

  it('opens the native ROM picker with a fixed filter', async () => {
    tauriMocks.open.mockResolvedValue('/private/test.gb')

    await expect(client.pickRom()).resolves.toBe('/private/test.gb')
    expect(tauriMocks.open).toHaveBeenCalledWith({
      directory: false,
      multiple: false,
      title: 'Open Game Boy ROM',
      filters: [{ name: 'Game Boy ROM', extensions: ['gb', 'gbc'] }]
    })
  })

  it('returns null when the native ROM picker is cancelled', async () => {
    tauriMocks.open.mockResolvedValue(null)

    await expect(client.pickRom()).resolves.toBeNull()
    expect(tauriMocks.invoke).not.toHaveBeenCalled()
  })

  it('maps lifecycle and input methods to exact Tauri commands', async () => {
    tauriMocks.invoke.mockResolvedValue(pausedSnapshot)

    await client.openRom('/private/test.gb')
    expect(tauriMocks.invoke).toHaveBeenLastCalledWith('open_rom', { path: '/private/test.gb' })

    tauriMocks.invoke.mockResolvedValue(undefined)
    await client.setKeyboardInput(['left', 'a'])
    expect(tauriMocks.invoke).toHaveBeenLastCalledWith('set_keyboard_input', { buttons: ['left', 'a'] })

    await client.acknowledgeFrame(7)
    expect(tauriMocks.invoke).toHaveBeenLastCalledWith('acknowledge_frame', { sequence: 7 })

    await client.setAudioGain(0.35)
    expect(tauriMocks.invoke).toHaveBeenLastCalledWith('set_audio_gain', { gain: 0.35 })
  })

  it('uses separate parsed-control and raw-frame channels', async () => {
    tauriMocks.invoke.mockResolvedValue(pausedSnapshot)
    const onSnapshot = vi.fn()
    const onFrame = vi.fn()

    await expect(client.subscribe({ onSnapshot, onFrame })).resolves.toMatchObject(pausedSnapshot)

    expect(tauriMocks.channels).toHaveLength(2)
    expect(tauriMocks.invoke).toHaveBeenCalledWith('subscribe_runtime', {
      events: tauriMocks.channels[0],
      frames: tauriMocks.channels[1]
    })

    tauriMocks.channels[0]?.onmessage({ type: 'snapshot', snapshot: pausedSnapshot })
    expect(onSnapshot).toHaveBeenCalledWith(expect.objectContaining({ phase: 'paused' }))

    tauriMocks.channels[1]?.onmessage(createFramePacket())
    expect(onFrame).toHaveBeenCalledWith(expect.objectContaining({ sequence: 7 }))
    expect(() => tauriMocks.channels[1]?.onmessage([0, 1, 2, 3])).toThrow('frame packet must be an ArrayBuffer')
  })

  it('preserves typed runtime errors and normalizes unknown failures', async () => {
    tauriMocks.invoke.mockRejectedValueOnce({ code: 'invalid-rom', message: 'bad header' })
    await expect(client.openRom('/private/bad.gb')).rejects.toEqual({
      code: 'invalid-rom',
      message: 'bad header'
    })

    tauriMocks.invoke.mockRejectedValueOnce('worker vanished')
    await expect(client.snapshot()).rejects.toEqual({
      code: 'runtime-unavailable',
      message: 'worker vanished'
    })
  })
})
