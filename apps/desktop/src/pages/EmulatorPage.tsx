import { useCallback, useEffect, useRef, useState } from 'react'
import {
  IconDeviceDesktop,
  IconFolderOpen,
  IconKeyboard,
  IconPlayerPause,
  IconPlayerPlay,
  IconRefresh,
  IconVolume,
  IconVolumeOff,
  IconX
} from '@tabler/icons-react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import {
  audioGainForPreferences,
  defaultEmulatorPreferences,
  type EmulatorPreferences,
  parseDisplayScale
} from '@/features/emulator/emulator-preferences'
import {
  type EmulatorPreferencesRepository,
  tauriEmulatorPreferencesRepository
} from '@/features/emulator/emulator-preferences-store'
import { GameViewport } from '@/features/emulator/GameViewport'
import { type EmulatorRuntimeClient, tauriEmulatorRuntimeClient } from '@/features/emulator/runtime-client'
import type { RuntimeErrorCode, RuntimePhase } from '@/features/emulator/runtime-types'
import { useEmulatorRuntime } from '@/features/emulator/use-emulator-runtime'
import { KeyboardMappingDialog } from '@/features/keyboard/KeyboardMappingDialog'
import { defaultKeyboardMapping, type KeyboardMapping } from '@/features/keyboard/keyboard-mapping'
import {
  type KeyboardMappingRepository,
  tauriKeyboardMappingRepository
} from '@/features/keyboard/keyboard-mapping-store'
import { useKeyboardInput } from '@/features/keyboard/use-keyboard-input'
import { RemoteControllerPanel } from '@/features/remote-controller/RemoteControllerPanel'
import type { RemoteSessionClient } from '@/features/remote-controller/remote-client'

const errorHeadings: Record<RuntimeErrorCode, string> = {
  'file-inaccessible': 'The ROM file could not be read',
  'invalid-rom': 'This file is not a valid Game Boy ROM',
  'cgb-only': 'Game Boy Color-only cartridges are not supported',
  'unsupported-mapper': 'This cartridge mapper is not supported',
  'core-failure': 'The emulator core stopped',
  'invalid-lifecycle': 'That action is not available',
  'runtime-unavailable': 'The desktop runtime is unavailable'
}

const phaseLabels: Record<RuntimePhase, string> = {
  empty: 'No ROM loaded',
  loading: 'Loading ROM',
  paused: 'Paused',
  running: 'Running',
  error: 'ROM error'
}

type EmulatorPageProps = {
  runtimeClient?: EmulatorRuntimeClient
  keyboardMappingRepository?: KeyboardMappingRepository
  emulatorPreferencesRepository?: EmulatorPreferencesRepository
  remoteSessionClient?: RemoteSessionClient
}

export const EmulatorPage = ({
  runtimeClient = tauriEmulatorRuntimeClient,
  keyboardMappingRepository = tauriKeyboardMappingRepository,
  emulatorPreferencesRepository = tauriEmulatorPreferencesRepository,
  remoteSessionClient
}: EmulatorPageProps) => {
  const runtime = useEmulatorRuntime(runtimeClient)
  const [mapping, setMapping] = useState<KeyboardMapping>(defaultKeyboardMapping)
  const [mappingDialogOpen, setMappingDialogOpen] = useState(false)
  const [controlsLoadFailed, setControlsLoadFailed] = useState(false)
  const [preferencesFailed, setPreferencesFailed] = useState(false)
  const [preferences, setPreferences] = useState<EmulatorPreferences>(defaultEmulatorPreferences)
  const preferencesRef = useRef<EmulatorPreferences>(defaultEmulatorPreferences)
  const preferencesRevisionRef = useRef(0)
  const { snapshot } = runtime
  const loading = snapshot.phase === 'loading'
  const loaded = snapshot.phase === 'paused' || snapshot.phase === 'running'

  useEffect(() => {
    let active = true
    void keyboardMappingRepository
      .load()
      .then((savedMapping) => {
        if (active) setMapping(savedMapping)
      })
      .catch(() => {
        if (active) setControlsLoadFailed(true)
      })
    return () => {
      active = false
    }
  }, [keyboardMappingRepository])

  useEffect(() => {
    let active = true
    const revision = preferencesRevisionRef.current
    void emulatorPreferencesRepository
      .load()
      .then(async (savedPreferences) => {
        if (!active || revision !== preferencesRevisionRef.current) return
        preferencesRef.current = savedPreferences
        setPreferences(savedPreferences)
        await runtime.setAudioGain(audioGainForPreferences(savedPreferences))
        if (active && revision === preferencesRevisionRef.current) setPreferencesFailed(false)
      })
      .catch(() => {
        if (active && revision === preferencesRevisionRef.current) setPreferencesFailed(true)
      })
    return () => {
      active = false
    }
  }, [emulatorPreferencesRepository, runtime.setAudioGain])

  const updatePreferences = useCallback(
    (patch: Partial<EmulatorPreferences>) => {
      const revision = preferencesRevisionRef.current + 1
      preferencesRevisionRef.current = revision
      const next = { ...preferencesRef.current, ...patch }
      preferencesRef.current = next
      setPreferences(next)
      void Promise.all([runtime.setAudioGain(audioGainForPreferences(next)), emulatorPreferencesRepository.save(next)])
        .then(() => {
          if (revision === preferencesRevisionRef.current) setPreferencesFailed(false)
        })
        .catch(() => {
          if (revision === preferencesRevisionRef.current) setPreferencesFailed(true)
        })
    },
    [emulatorPreferencesRepository, runtime.setAudioGain]
  )

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat || mappingDialogOpen) return
      const target = event.target
      if (
        target instanceof HTMLElement &&
        (target.matches('input, textarea, select, [contenteditable="true"]') ||
          target.closest('input, textarea, select, [contenteditable="true"]'))
      ) {
        return
      }
      const commandModifier = event.metaKey || event.ctrlKey
      if (commandModifier && !event.shiftKey && event.code === 'KeyO' && !loading) {
        event.preventDefault()
        void runtime.openRom()
      } else if (commandModifier && event.shiftKey && event.code === 'KeyR' && loaded) {
        event.preventDefault()
        void runtime.restart()
      } else if (!commandModifier && !event.altKey && event.code === 'Space' && loaded) {
        event.preventDefault()
        void (snapshot.phase === 'running' ? runtime.pause() : runtime.start())
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [
    loaded,
    loading,
    mappingDialogOpen,
    runtime.openRom,
    runtime.pause,
    runtime.restart,
    runtime.start,
    snapshot.phase
  ])

  useKeyboardInput({
    mapping,
    enabled: snapshot.phase === 'running',
    suspended: mappingDialogOpen,
    setKeyboardInput: runtime.setKeyboardInput
  })

  return (
    <main className="desktop-shell">
      <Card className="emulator-card" aria-labelledby="emulator-title">
        <CardHeader className="app-header">
          <div>
            <p className="eyebrow">Desktop emulator</p>
            <h1 id="emulator-title">Game Boy</h1>
          </div>
          <Badge aria-live="polite" variant="secondary" className="status-badge">
            {phaseLabels[snapshot.phase]}
          </Badge>
        </CardHeader>

        <CardContent>
          <GameViewport
            phase={snapshot.phase}
            error={snapshot.error}
            subscribeFrames={runtime.subscribeFrames}
            acknowledgeFrame={runtime.acknowledgeFrame}
            displayScale={preferences.displayScale}
          />

          {snapshot.rom && (
            <section className="rom-summary" aria-label="Loaded ROM">
              <strong>{snapshot.rom.fileName}</strong>
              <span>{snapshot.rom.title}</span>
              <Badge variant="outline">{snapshot.rom.mapper}</Badge>
            </section>
          )}

          {snapshot.error && (
            <Alert variant="destructive" className="runtime-alert">
              <AlertTitle>{errorHeadings[snapshot.error.code]}</AlertTitle>
              <AlertDescription>{snapshot.error.message}</AlertDescription>
            </Alert>
          )}

          {controlsLoadFailed && (
            <Alert className="runtime-alert">
              <AlertDescription>Controls could not be loaded. Default keys are active.</AlertDescription>
            </Alert>
          )}

          {preferencesFailed && (
            <Alert className="runtime-alert">
              <AlertDescription>
                Emulator preferences could not be loaded or saved. Current session settings remain active, but may not
                persist.
              </AlertDescription>
            </Alert>
          )}

          <section className="emulator-command-panel" aria-labelledby="rom-controls-title">
            <div className="section-heading">
              <div>
                <p className="eyebrow">ROM</p>
                <h2 id="rom-controls-title">Emulator controls</h2>
              </div>
              <span className="shortcut-hint">⌘/Ctrl O · Space · ⇧⌘/Ctrl R</span>
            </div>
            <fieldset className="lifecycle-actions">
              <legend className="sr-only">ROM lifecycle</legend>
              <Button type="button" onClick={() => void runtime.openRom()} disabled={loading}>
                <IconFolderOpen aria-hidden="true" size={18} />
                Open ROM
              </Button>
              <Button
                type="button"
                variant="secondary"
                onClick={() => void (snapshot.phase === 'running' ? runtime.pause() : runtime.start())}
                disabled={!loaded}
              >
                {snapshot.phase === 'running' ? (
                  <IconPlayerPause aria-hidden="true" size={18} />
                ) : (
                  <IconPlayerPlay aria-hidden="true" size={18} />
                )}
                {snapshot.phase === 'running' ? 'Pause' : 'Resume'}
              </Button>
              <Button type="button" variant="secondary" onClick={() => void runtime.restart()} disabled={!loaded}>
                <IconRefresh aria-hidden="true" size={18} />
                Restart
              </Button>
              <Button type="button" variant="secondary" onClick={() => void runtime.close()} disabled={!loaded}>
                <IconX aria-hidden="true" size={18} />
                Close ROM
              </Button>
            </fieldset>
          </section>

          <section className="emulator-settings" aria-labelledby="emulator-settings-title">
            <div className="section-heading">
              <div>
                <p className="eyebrow">Local preferences</p>
                <h2 id="emulator-settings-title">Audio and display</h2>
              </div>
            </div>
            <div className="settings-grid">
              <fieldset className="setting-group audio-settings">
                <legend>
                  <IconVolume aria-hidden="true" size={18} /> Audio
                </legend>
                <div className="volume-control">
                  <Button
                    type="button"
                    variant="secondary"
                    aria-pressed={preferences.muted}
                    onClick={() => updatePreferences({ muted: !preferences.muted })}
                  >
                    {preferences.muted ? (
                      <IconVolumeOff aria-hidden="true" size={18} />
                    ) : (
                      <IconVolume aria-hidden="true" size={18} />
                    )}
                    {preferences.muted ? 'Unmute' : 'Mute'}
                  </Button>
                  <label htmlFor="emulator-volume">Volume</label>
                  <input
                    id="emulator-volume"
                    type="range"
                    min="0"
                    max="100"
                    step="1"
                    value={preferences.volumePercent}
                    onChange={(event) => updatePreferences({ volumePercent: Number(event.currentTarget.value) })}
                  />
                  <output htmlFor="emulator-volume">{preferences.volumePercent}%</output>
                </div>
              </fieldset>

              <fieldset className="setting-group display-settings">
                <legend>
                  <IconDeviceDesktop aria-hidden="true" size={18} /> Display
                </legend>
                <label htmlFor="display-scale">Screen size</label>
                <select
                  id="display-scale"
                  value={preferences.displayScale}
                  onChange={(event) => {
                    const value = event.currentTarget.value
                    updatePreferences({ displayScale: parseDisplayScale(value === 'fit' ? value : Number(value)) })
                  }}
                >
                  <option value="1">1× · 160 × 144</option>
                  <option value="2">2× · 320 × 288</option>
                  <option value="3">3× · 480 × 432</option>
                  <option value="4">4× · 640 × 576</option>
                  <option value="fit">Fit available space</option>
                </select>
                <Button type="button" variant="secondary" onClick={() => setMappingDialogOpen(true)}>
                  <IconKeyboard aria-hidden="true" size={18} />
                  Keyboard controls
                </Button>
              </fieldset>
            </div>
          </section>
        </CardContent>

        <Separator />

        <CardFooter className="remote-footer">
          <RemoteControllerPanel client={remoteSessionClient} />
        </CardFooter>
      </Card>

      <KeyboardMappingDialog
        open={mappingDialogOpen}
        mapping={mapping}
        repository={keyboardMappingRepository}
        onOpenChange={setMappingDialogOpen}
        onSave={setMapping}
      />
    </main>
  )
}
