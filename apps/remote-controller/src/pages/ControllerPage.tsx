import { PROTOCOL_VERSION } from '@gameboy/protocol'
import { IconWifiOff } from '@tabler/icons-react'

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
      <div className="connection-state" role="status">
        <IconWifiOff aria-hidden="true" size={18} />
        Disconnected
      </div>
    </header>

    <section className="controls" aria-label="Game Boy controls">
      <fieldset className="d-pad">
        <legend className="sr-only">Directional controls</legend>
        {dPadButtons.map((label) => (
          <button key={label} type="button" className={`control-button direction ${label.toLowerCase()}`} disabled>
            {label}
          </button>
        ))}
      </fieldset>

      <div className="menu-controls">
        {menuButtons.map((label) => (
          <button key={label} type="button" className="control-button menu" disabled>
            {label}
          </button>
        ))}
      </div>

      <fieldset className="action-controls">
        <legend className="sr-only">Action controls</legend>
        {actionButtons.map((label) => (
          <button key={label} type="button" className={`control-button action action-${label.toLowerCase()}`} disabled>
            {label}
          </button>
        ))}
      </fieldset>
    </section>

    <footer>Protocol {PROTOCOL_VERSION}</footer>
  </main>
)
