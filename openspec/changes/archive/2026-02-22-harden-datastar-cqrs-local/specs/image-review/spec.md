## MODIFIED Requirements

### Requirement: Apply decision to current image
The system MUST apply the left or right action to the currently selected image when the user presses the corresponding directional key bindings or clicks the corresponding half of the stack view. Rapid consecutive key commands MUST preserve user intent by dispatching each command without client-side auto-cancellation.

#### Scenario: Left decision applied
- **WHEN** the user presses `ArrowLeft` or `h`, or clicks the left half of the stack view
- **THEN** the configured left action is applied to the current image and the UI advances according to navigation rules

#### Scenario: Right decision applied
- **WHEN** the user presses `ArrowRight` or `l`, or clicks the right half of the stack view
- **THEN** the configured right action is applied to the current image and the UI advances according to navigation rules

#### Scenario: Rapid keyboard decisions preserve command dispatch
- **WHEN** the user presses multiple decision/navigation keys rapidly
- **THEN** each corresponding command request is dispatched and not auto-canceled by frontend request cancellation policy
