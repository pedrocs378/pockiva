import { useEffect, useState } from 'react'
import { PROTOCOL_VERSION } from '@gameboy/protocol'
import {
  IconDeviceMobile,
  IconFolderOpen,
  IconPlayerPause,
  IconPlayerPlay,
  IconRefresh,
  IconX
} from '@tabler/icons-react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
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
}

export const EmulatorPage = ({
  runtimeClient = tauriEmulatorRuntimeClient,
  keyboardMappingRepository = tauriKeyboardMappingRepository
}: EmulatorPageProps) => {
  const runtime = useEmulatorRuntime(runtimeClient)
  const [mapping, setMapping] = useState<KeyboardMapping>(defaultKeyboardMapping)
  const [mappingDialogOpen, setMappingDialogOpen] = useState(false)
  const [settingsLoadFailed, setSettingsLoadFailed] = useState(false)
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
        if (active) setSettingsLoadFailed(true)
      })
    return () => {
      active = false
    }
  }, [keyboardMappingRepository])

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

          {settingsLoadFailed && (
            <Alert className="runtime-alert">
              <AlertDescription>Controls could not be loaded. Default keys are active.</AlertDescription>
            </Alert>
          )}

          <fieldset className="lifecycle-actions">
            <legend className="sr-only">Emulator controls</legend>
            <Button type="button" onClick={() => void runtime.openRom()} disabled={loading}>
              <IconFolderOpen aria-hidden="true" size={18} />
              Open ROM
            </Button>
            <Button
              type="button"
              variant="secondary"
              onClick={() => void runtime.start()}
              disabled={snapshot.phase !== 'paused'}
            >
              <IconPlayerPlay aria-hidden="true" size={18} />
              Start
            </Button>
            <Button
              type="button"
              variant="secondary"
              onClick={() => void runtime.pause()}
              disabled={snapshot.phase !== 'running'}
            >
              <IconPlayerPause aria-hidden="true" size={18} />
              Pause
            </Button>
            <Button type="button" variant="secondary" onClick={() => void runtime.restart()} disabled={!loaded}>
              <IconRefresh aria-hidden="true" size={18} />
              Restart
            </Button>
            <Button type="button" variant="secondary" onClick={() => void runtime.close()} disabled={!loaded}>
              <IconX aria-hidden="true" size={18} />
              Close ROM
            </Button>
            <Button type="button" variant="secondary" onClick={() => setMappingDialogOpen(true)}>
              Keyboard controls
            </Button>
          </fieldset>
        </CardContent>

        <Separator />

        <CardFooter className="remote-footer">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <button aria-disabled="true" className="remote-status" type="button">
                  <IconDeviceMobile aria-hidden="true" size={20} />
                  <div>
                    <strong>Mobile controller is off</strong>
                    <span>Remote protocol {PROTOCOL_VERSION}</span>
                  </div>
                </button>
              </TooltipTrigger>
              <TooltipContent>PED-39 enables remote sessions.</TooltipContent>
            </Tooltip>
          </TooltipProvider>
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
