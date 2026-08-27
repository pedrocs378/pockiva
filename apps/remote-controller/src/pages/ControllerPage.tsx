import { PROTOCOL_VERSION } from '@gameboy/protocol'
import { IconWifiOff } from '@tabler/icons-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'

const dPadButtons = ['Up', 'Left', 'Right', 'Down'] as const
const actionButtons = ['B', 'A'] as const
const menuButtons = ['Select', 'Start'] as const

export const ControllerPage = () => (
  <main className="controller-shell">
    <header className="controller-header">
      <div>
        <p>Remote input</p>
        <h1>Game Boy Controller</h1>
      </div>
      <Badge variant="outline" className="connection-state" role="status">
        <IconWifiOff aria-hidden="true" size={18} />
        Disconnected
      </Badge>
    </header>

    <section className="controls" aria-label="Game Boy controls">
      <fieldset className="d-pad">
        <legend className="sr-only">Directional controls</legend>
        {dPadButtons.map((label) => (
          <Button
            key={label}
            type="button"
            variant="unstyled"
            size="auto"
            className={`control-button direction ${label.toLowerCase()}`}
            disabled
          >
            {label}
          </Button>
        ))}
      </fieldset>

      <div className="menu-controls">
        {menuButtons.map((label) => (
          <Button key={label} type="button" variant="unstyled" size="auto" className="control-button menu" disabled>
            {label}
          </Button>
        ))}
      </div>

      <fieldset className="action-controls">
        <legend className="sr-only">Action controls</legend>
        {actionButtons.map((label) => (
          <Button
            key={label}
            type="button"
            variant="unstyled"
            size="auto"
            className={`control-button action action-${label.toLowerCase()}`}
            disabled
          >
            {label}
          </Button>
        ))}
      </fieldset>
    </section>

    <footer>Protocol {PROTOCOL_VERSION}</footer>
  </main>
)
