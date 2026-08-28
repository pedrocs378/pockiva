import { useMemo, useState } from 'react'
import { PROTOCOL_VERSION } from '@gameboy/protocol'
import { IconWifi, IconWifiOff } from '@tabler/icons-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ControllerButton } from '@/features/controller/ControllerButton'
import { DirectionalControl } from '@/features/controller/DirectionalControl'
import {
  browserDirectionalModeRepository,
  type DirectionalMode,
  type DirectionalModeRepository
} from '@/features/controller/directional-mode'
import { useControllerInput } from '@/features/controller/use-controller-input'
import { parsePairingUrl } from '@/features/session/pairing'
import { createWebSocketTransport, type SessionTransport } from '@/features/session/transport'
import { useControllerSession } from '@/features/session/use-controller-session'
import { ACTION_BUTTONS, BUTTON_LABELS, D_PAD_BUTTONS, MENU_BUTTONS } from '@/constants/controller'

const STATUS_COPY = {
  connecting: 'Connecting',
  connected: 'Connected',
  disconnected: 'Disconnected',
  'missing-token': 'Pairing link required',
  'expired-token': 'Pairing link expired',
  'incompatible-protocol': 'Protocol mismatch',
  'controller-in-use': 'Another controller is connected',
  'server-unavailable': 'Server unavailable'
} as const

const STATUS_HELP = {
  connecting: 'Establishing a local controller session…',
  connected: 'Ready for touch input.',
  disconnected: 'The controller is disconnected.',
  'missing-token': 'Scan the QR Code shown by the desktop app.',
  'expired-token': 'Start a new desktop session and scan its QR Code.',
  'incompatible-protocol': 'Update the desktop and controller to the same version.',
  'controller-in-use': 'Disconnect the active phone before trying again.',
  'server-unavailable': 'Check that the desktop session is still running on this network.'
} as const

export type ControllerPageProps = {
  pairingUrl?: URL
  transport?: SessionTransport
  directionalModeRepository?: DirectionalModeRepository
}

export const ControllerPage = ({
  pairingUrl,
  transport,
  directionalModeRepository = browserDirectionalModeRepository
}: ControllerPageProps) => {
  const [directionalMode, setDirectionalMode] = useState(() => directionalModeRepository.load())
  const pairing = useMemo(() => parsePairingUrl(pairingUrl ?? new URL(window.location.href)), [pairingUrl])
  const resolvedTransport = useMemo(() => transport ?? createWebSocketTransport(), [transport])
  const controller = useControllerSession(pairing, resolvedTransport)
  const input = useControllerInput(controller.session)
  const status = controller.snapshot.status
  const controlsDisabled = status !== 'connected'
  const disconnect = () => {
    controller.disconnect()
    input.releaseAll()
  }
  const changeDirectionalMode = (mode: DirectionalMode) => {
    input.releaseButtons(D_PAD_BUTTONS)
    directionalModeRepository.save(mode)
    setDirectionalMode(mode)
  }

  return (
    <main className="controller-shell">
      <header className="controller-header">
        <div className="controller-title">
          <p>Remote input</p>
          <h1>Game Boy Controller</h1>
        </div>
        <div className="session-summary">
          <Badge variant="outline" className="connection-state" role="status">
            {status === 'connected' ? (
              <IconWifi aria-hidden="true" size={18} />
            ) : (
              <IconWifiOff aria-hidden="true" size={18} />
            )}
            {STATUS_COPY[status]}
          </Badge>
          <p className="session-help">{STATUS_HELP[status]}</p>
          {status === 'connected' || status === 'connecting' ? (
            <Button type="button" variant="secondary" size="sm" onClick={disconnect}>
              Disconnect
            </Button>
          ) : status === 'disconnected' ? (
            <Button type="button" variant="secondary" size="sm" onClick={controller.connect}>
              Connect
            </Button>
          ) : status === 'server-unavailable' ? (
            <Button type="button" variant="secondary" size="sm" onClick={controller.connect}>
              Retry
            </Button>
          ) : null}
        </div>
      </header>

      <section className="controls" aria-label="Game Boy controls">
        <DirectionalControl
          mode={directionalMode}
          disabled={controlsDisabled}
          pressedButtons={input.pressedButtons}
          onModeChange={changeDirectionalMode}
          pressPointer={input.pressPointer}
          setPointerButtons={input.setPointerButtons}
          releasePointer={input.releasePointer}
        />

        <div className="menu-controls">
          {MENU_BUTTONS.map((button) => (
            <ControllerButton
              key={button}
              button={button}
              label={BUTTON_LABELS[button]}
              className="control-button menu"
              pressed={input.pressedButtons.has(button)}
              disabled={controlsDisabled}
              onPress={input.pressPointer}
              onRelease={input.releasePointer}
            />
          ))}
        </div>

        <fieldset className="action-controls">
          <legend className="sr-only">Action controls</legend>
          {ACTION_BUTTONS.map((button) => (
            <ControllerButton
              key={button}
              button={button}
              label={BUTTON_LABELS[button]}
              className={`control-button action action-${button}`}
              pressed={input.pressedButtons.has(button)}
              disabled={controlsDisabled}
              onPress={input.pressPointer}
              onRelease={input.releasePointer}
            />
          ))}
        </fieldset>
      </section>

      <footer>Protocol {PROTOCOL_VERSION}</footer>
    </main>
  )
}
