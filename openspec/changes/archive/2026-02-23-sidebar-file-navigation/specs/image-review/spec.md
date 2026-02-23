## MODIFIED Requirements

### Requirement: Single-image presentation
The system MUST present exactly one supported image file at a time to the user for review. Directory entries MUST NOT be treated as reviewable images.

#### Scenario: Viewing the current image
- **WHEN** the user is in the viewing state
- **THEN** the UI displays the currently selected image file and its position in the image list

#### Scenario: Directory entries are excluded from review presentation
- **WHEN** directory entries exist in the current working directory
- **THEN** those directories are not included as selectable items in the image-viewer list

### Requirement: Open new path
The system MUST allow users to change the current working directory from the sidebar navigation surface while enforcing the root-directory boundary established at launch. `Ctrl+O` MUST toggle this sidebar surface instead of opening a native picker.

#### Scenario: Toggle sidebar navigation
- **WHEN** the user presses `Ctrl+O`
- **THEN** the sidebar file navigation is shown or hidden

#### Scenario: Open a directory from sidebar
- **WHEN** the user selects a directory row and chooses `open`
- **THEN** the current run is updated to the selected child directory and the image list is rebuilt from supported files in that directory

### Requirement: Apply decision to current image
The system MUST apply left/right actions only when the current selection is an image file.

#### Scenario: Left decision applied to selected image file
- **WHEN** the selected row is an image file and the user presses `ArrowLeft` or `h`, or clicks the left half of the stack view
- **THEN** the configured left action is applied to the current image and the UI advances according to navigation rules

#### Scenario: Right decision applied to selected image file
- **WHEN** the selected row is an image file and the user presses `ArrowRight` or `l`, or clicks the right half of the stack view
- **THEN** the configured right action is applied to the current image and the UI advances according to navigation rules

#### Scenario: Directional action on selected directory does not apply image decision
- **WHEN** the selected row is a directory and the user presses a directional action key
- **THEN** the system performs directory navigation behavior and does not apply an image decision

### Requirement: Keyboard shortcuts for queue, list, help, and close
The system MUST provide keyboard shortcuts for opening queue/help views, toggling sidebar file navigation, and closing the top modal or menu surface.

#### Scenario: Open queue, sidebar navigation, and help
- **WHEN** the user presses `q`, `Ctrl+O`, or `?`
- **THEN** the system opens the queue view, toggles sidebar file navigation, or opens help respectively

#### Scenario: Close top modal or menu surface
- **WHEN** the user presses `Escape`
- **THEN** the system closes the top modal/menu surface in the active stack
