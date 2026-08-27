import { PROTOCOL_VERSION } from '@gameboy/protocol'
import { IconDeviceMobile, IconFolderOpen } from '@tabler/icons-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'

export const EmulatorPage = () => (
  <main className="desktop-shell">
    <section className="emulator-card" aria-labelledby="emulator-title">
      <header className="app-header">
        <div>
          <p className="eyebrow">Desktop emulator</p>
          <h1 id="emulator-title">Game Boy</h1>
        </div>
        <Badge variant="secondary" className="status-badge">
          No ROM loaded
        </Badge>
      </header>

      <div className="viewport" role="img" aria-label="Game display">
        <span>160 × 144</span>
      </div>

      <div className="foundation-actions">
        <Button type="button" variant="unstyled" size="auto" className="primary-button" disabled>
          <IconFolderOpen aria-hidden="true" size={18} />
          Open ROM
        </Button>

        <div className="remote-status">
          <IconDeviceMobile aria-hidden="true" size={20} />
          <div>
            <strong>Mobile controller is off</strong>
            <span>Remote protocol {PROTOCOL_VERSION}</span>
          </div>
        </div>
      </div>
    </section>
  </main>
)
