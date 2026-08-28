# Remote controller landscape and joystick design

Date: 2026-08-28
Parent: PED-32

## Objective

Improve the mobile remote controller for landscape play by enlarging the directional and action controls and by offering a locally configurable fixed virtual joystick as an alternative to the existing D-pad.

## Approved product decisions

- Keep the existing D-pad available.
- Add a fixed-position virtual joystick rather than a floating joystick.
- Support eight digital directions, including simultaneous horizontal and vertical buttons for diagonals.
- Persist the selected directional mode on the phone and restore it on the next visit.
- Enlarge the directional control and A/B buttons in landscape while preserving safe areas and preventing overlap.
- Keep the current network protocol and Game Boy core contracts unchanged.

## Component boundaries

### Directional control

The controller page delegates the left-side input surface to a directional-control component. It renders either the current four-button D-pad or the fixed joystick according to the saved preference. Switching modes first releases every active directional button, then replaces the surface.

### Fixed joystick

The joystick owns one active pointer at a time. It captures that pointer, measures its position relative to the fixed center, and renders the knob within a bounded circular travel area. Other pointers remain available for A, B, Start, and Select.

A pure direction resolver receives the pointer vector and the joystick geometry. It applies a central dead zone and maps the remaining angle to one of eight digital results:

- up;
- up + right;
- right;
- down + right;
- down;
- down + left;
- left;
- up + left.

The controller-input tracker is extended from one button per pointer to a set of buttons per pointer. A `setPointerButtons` operation replaces the set for one pointer and returns the aggregate button transitions caused by that replacement. Existing single-button controls continue to use a convenience operation over the same tracker.

The joystick sends only the difference between its previous and next directional sets through this internal API. The session still receives the existing individual digital button transitions, so no analog values or protocol changes cross the network boundary. Aggregating all pointer sets before emitting transitions prevents one pointer from releasing a button that another pointer still holds.

### Controller preference

The directional mode is stored in browser `localStorage` under a versioned key. A small repository validates external data at load time. Missing or malformed data falls back to `d-pad`; a malformed persisted value is replaced with the safe default.

The preference is local to the phone. It is not sent to the desktop, network session, protocol package, or emulator core.

## Input lifecycle and safety

The joystick releases all of its directions when any of the following occurs:

- the pointer is released;
- the pointer is cancelled;
- pointer capture is lost;
- the controller switches between D-pad and joystick;
- the session disconnects;
- the page becomes hidden or unloads.

Moving back into the dead zone also releases the current direction. If a pointer begins outside the joystick surface or a second pointer touches the joystick while one is active, the joystick ignores it.

The existing session-level release and state synchronization remain the final defense against stuck input.

## Layout

The directional-mode selector is an accessible segmented control labeled `D-pad` and `Joystick`. It remains visible on the controller and can be changed during a connected session.

Portrait mode preserves the current control hierarchy and accommodates the selector without reducing the existing minimum touch targets.

Landscape mode compacts secondary header content so the gameplay surface receives more height. The D-pad or joystick grows on the left, A/B grow proportionally on the right, and Start/Select remain centered. CSS orientation and viewport media queries control the layout; JavaScript orientation detection is not required. All sizing respects `env(safe-area-inset-*)`, short landscape viewports, and the existing 320-pixel minimum width.

## Accessibility

- The mode selector uses native controls with an explicit accessible group label and selected state.
- The D-pad retains individually named buttons.
- The joystick exposes a directional-control label and an observable pressed state, while pointer movement remains the primary interaction.
- Disabled connection states disable both directional modes consistently.
- Visible and effective touch targets remain at least 44 by 44 CSS pixels where the viewport permits.
- Essential state and actions do not depend on hover.

## Testing

### Unit tests

- Dead-zone vectors resolve to no direction.
- Cardinal vectors resolve to one button.
- Diagonal sectors resolve to two compatible buttons.
- Boundary vectors have deterministic results.
- Preference loading covers missing, valid, and malformed stored data.

### Component tests

- Pointer down, move, up, cancel, and lost capture update and release directions correctly.
- Moving between sectors sends only the required button changes.
- Overlapping button sets from different pointers preserve a button until its final owning pointer releases it.
- Returning to the dead zone releases the direction.
- A second joystick pointer is ignored while the first remains active.
- Switching modes releases active directional input.
- A/B can remain pressed by separate pointers while the joystick changes direction.

### Page and responsive tests

- The saved mode is restored and a mode change is persisted.
- Both directional modes respect disconnected and error states.
- Portrait and landscape media rules remain present with safe-area handling.
- Landscape rules provide larger directional and action-control bounds than the current compact layout without clipping short viewports.

### Manual acceptance

Validate in responsive browser dimensions and on a real phone connected to the desktop. Exercise D-pad, joystick, all eight directions, A/B multitouch, mode switching, rotation during a session, disconnect/reconnect, and reopening the controller to confirm persistence.

## Non-goals

- Analog input or protocol changes.
- Haptic feedback.
- Floating or repositionable joystick bases.
- Per-game controller profiles.
- Desktop-side synchronization of the phone's directional-mode preference.
