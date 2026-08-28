import type { Button } from '@gameboy/protocol'
import { BUTTON_LABELS, D_PAD_BUTTONS } from '@/constants/controller'
import { ControllerButton } from './ControllerButton'
import type { DirectionalMode } from './directional-mode'
import { VirtualJoystick } from './VirtualJoystick'

export type DirectionalControlProps = {
  mode: DirectionalMode
  disabled: boolean
  pressedButtons: ReadonlySet<Button>
  onModeChange: (mode: DirectionalMode) => void
  pressPointer: (pointerId: number, button: Button) => void
  setPointerButtons: (pointerId: number, buttons: readonly Button[]) => void
  releasePointer: (pointerId: number) => void
}

const directionalModes: readonly DirectionalMode[] = ['d-pad', 'joystick']

export const DirectionalControl = ({
  mode,
  disabled,
  pressedButtons,
  onModeChange,
  pressPointer,
  setPointerButtons,
  releasePointer
}: DirectionalControlProps) => (
  <div className="directional-control">
    <fieldset className="direction-mode-selector">
      <legend className="sr-only">Directional control</legend>
      <div role="radiogroup" aria-label="Directional control">
        {directionalModes.map((option) => (
          <label key={option}>
            <input
              type="radio"
              name="directional-mode"
              value={option}
              checked={mode === option}
              onChange={() => onModeChange(option)}
            />
            <span>{option === 'd-pad' ? 'D-pad' : 'Joystick'}</span>
          </label>
        ))}
      </div>
    </fieldset>
    {mode === 'd-pad' ? (
      <fieldset className="d-pad">
        <legend className="sr-only">Directional controls</legend>
        {D_PAD_BUTTONS.map((button) => (
          <ControllerButton
            key={button}
            button={button}
            label={BUTTON_LABELS[button]}
            className={`control-button direction ${button}`}
            pressed={pressedButtons.has(button)}
            disabled={disabled}
            onPress={pressPointer}
            onRelease={releasePointer}
          />
        ))}
      </fieldset>
    ) : (
      <VirtualJoystick disabled={disabled} setPointerButtons={setPointerButtons} releasePointer={releasePointer} />
    )}
  </div>
)
