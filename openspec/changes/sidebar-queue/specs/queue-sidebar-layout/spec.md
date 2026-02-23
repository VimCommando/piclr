## ADDED Requirements

### Requirement: Queue sidebar presentation
The system MUST present the queue as a slide-out sidebar list instead of a modal, and the queue sidebar MUST be rendered between the header and footer regions.

#### Scenario: Queue opens as sidebar
- **WHEN** the user opens the queue
- **THEN** the queue is displayed as a slide-out sidebar list and no queue modal is shown

#### Scenario: Queue stays within header/footer bounds
- **WHEN** the queue sidebar is visible
- **THEN** its rendered bounds remain between the header and footer elements

### Requirement: Queue visibility controls
The system MUST toggle queue sidebar visibility when the user presses `q` or clicks the queue icon.

#### Scenario: Keyboard toggle for queue sidebar
- **WHEN** the user presses `q`
- **THEN** queue sidebar visibility toggles

#### Scenario: Queue icon toggles sidebar
- **WHEN** the user clicks the queue icon
- **THEN** queue sidebar visibility toggles

### Requirement: Queue header controls placement
The system MUST render queue action controls at the top of the queue list, consistent with the control placement pattern used in the file list.

#### Scenario: Queue controls appear above list items
- **WHEN** the queue sidebar is visible
- **THEN** queue action controls are shown at the top of the queue list before queue items

### Requirement: Selected queue item action affordances
The system MUST show apply and undo action icons on the currently selected queue item.

#### Scenario: Selected row displays apply and undo icons
- **WHEN** a queue item is selected in the queue sidebar
- **THEN** that selected item displays apply and undo icons
