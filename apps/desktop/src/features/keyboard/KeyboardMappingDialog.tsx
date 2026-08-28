import { useEffect, useState } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { type RuntimeButton, runtimeButtons } from '@/features/emulator/runtime-types'
import {
  defaultKeyboardMapping,
  getKeyboardCodeLabel,
  getRuntimeButtonLabel,
  type KeyboardMapping,
  parseKeyboardMapping,
  remapButton
} from './keyboard-mapping'

type MappingWriter = {
  save(mapping: KeyboardMapping): Promise<void>
}

type KeyboardMappingDialogProps = {
  open: boolean
  mapping: KeyboardMapping
  repository: MappingWriter
  onOpenChange: (open: boolean) => void
  onSave: (mapping: KeyboardMapping) => void
}

export const KeyboardMappingDialog = ({
  open,
  mapping,
  repository,
  onOpenChange,
  onSave
}: KeyboardMappingDialogProps) => {
  const [draft, setDraft] = useState<KeyboardMapping>(mapping)
  const [capturing, setCapturing] = useState<RuntimeButton | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (!open) return
    setDraft(mapping)
    setCapturing(null)
    setError(null)
  }, [mapping, open])

  const captureKey = (event: React.KeyboardEvent) => {
    if (!capturing) return
    event.preventDefault()
    event.stopPropagation()
    try {
      setDraft(remapButton(draft, capturing, event.code))
      setCapturing(null)
      setError(null)
    } catch (captureError) {
      setError(captureError instanceof Error ? captureError.message : 'That key cannot be assigned.')
    }
  }

  const save = async () => {
    setSaving(true)
    setError(null)
    try {
      const validated = parseKeyboardMapping(draft)
      await repository.save(validated)
      onSave(validated)
      onOpenChange(false)
    } catch {
      setError('Controls could not be saved.')
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent onKeyDownCapture={captureKey}>
        <DialogHeader>
          <DialogTitle>Keyboard controls</DialogTitle>
          <DialogDescription>Choose one physical key for each Game Boy button.</DialogDescription>
        </DialogHeader>

        <div className="keyboard-mapping-grid">
          {runtimeButtons.map((button) => {
            const label = getRuntimeButtonLabel(button)
            const binding = capturing === button ? 'Press a key…' : getKeyboardCodeLabel(draft[button])
            return (
              <div className="keyboard-mapping-row" key={button}>
                <Label htmlFor={`binding-${button}`}>{label}</Label>
                <Button
                  id={`binding-${button}`}
                  type="button"
                  variant="secondary"
                  aria-label={capturing === button ? binding : `${label}: ${binding}`}
                  onClick={() => {
                    setCapturing(button)
                    setError(null)
                  }}
                >
                  {binding}
                </Button>
              </div>
            )
          })}
        </div>

        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <DialogFooter className="keyboard-dialog-actions">
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            type="button"
            variant="secondary"
            onClick={() => {
              setDraft(defaultKeyboardMapping)
              setCapturing(null)
              setError(null)
            }}
          >
            Restore defaults
          </Button>
          <Button type="button" disabled={saving} onClick={() => void save()}>
            Save controls
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
