import { z } from 'zod'

export const displayScales = ['fit', 1, 2, 3, 4] as const

export type DisplayScale = (typeof displayScales)[number]

const displayScaleSchema = z.union([z.literal('fit'), z.literal(1), z.literal(2), z.literal(3), z.literal(4)])

export type EmulatorPreferences = {
  volumePercent: number
  muted: boolean
  displayScale: DisplayScale
}

const preferencesSchema = z.strictObject({
  volumePercent: z.number().int().min(0).max(100),
  muted: z.boolean(),
  displayScale: displayScaleSchema
})

export const defaultEmulatorPreferences: EmulatorPreferences = Object.freeze({
  volumePercent: 100,
  muted: false,
  displayScale: 3
})

export const parseEmulatorPreferences = (value: unknown): EmulatorPreferences => preferencesSchema.parse(value)

export const parseDisplayScale = (value: unknown): DisplayScale => displayScaleSchema.parse(value)

export const audioGainForPreferences = ({ muted, volumePercent }: EmulatorPreferences): number =>
  muted ? 0 : volumePercent / 100
