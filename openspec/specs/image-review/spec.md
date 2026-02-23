## Purpose
TBD

## Requirements

### Requirement: Scan and list images
The system MUST scan a user-specified directory and build an in-memory list of image files to review for the current run.

#### Scenario: Directory scan succeeds
- **WHEN** the user provides a valid directory path
- **THEN** the system lists all supported image files in that directory for review

### Requirement: Supported image formats
The system MUST support major web-compatible image formats: jpg/jpeg, png, gif, webp, and heic.

#### Scenario: Supported formats are included
- **WHEN** the directory contains images in supported formats
- **THEN** those images are included in the review list

### Requirement: Skip unreadable images
The system MUST skip images that cannot be read due to permissions or I/O errors and MUST log a warning for each skipped image.

#### Scenario: Unreadable file is skipped
- **WHEN** a file cannot be read during scan
- **THEN** the file is excluded from the review list and a warning is logged

### Requirement: Sorting options
The system MUST support ordering the review list by filesystem order and by configurable sort keys: created_at, last_modified, and alphabetical, each with ascending or descending direction.

#### Scenario: User selects a sort option
- **WHEN** the user selects a sort key and direction
- **THEN** the review list is reordered accordingly and the current selection remains valid

### Requirement: Single-image presentation
The system MUST present exactly one supported image file at a time to the user for review. Directory entries MUST NOT be treated as reviewable images.

#### Scenario: Viewing the current image
- **WHEN** the user is in the viewing state
- **THEN** the UI displays the currently selected image file and its position in the image list

#### Scenario: Directory entries are excluded from review presentation
- **WHEN** directory entries exist in the current working directory
- **THEN** those directories are not included as selectable items in the image-viewer list

### Requirement: Preload adjacent images
The system MUST preload the next image in the current review order to minimize navigation latency.

#### Scenario: Preload next image
- **WHEN** an image is displayed
- **THEN** the next image is preloaded for faster navigation

### Requirement: EXIF orientation
The system MUST display images using EXIF orientation metadata when present.

#### Scenario: Image with orientation metadata
- **WHEN** the current image contains EXIF orientation metadata
- **THEN** the image is displayed with the correct orientation

### Requirement: Navigation without state changes
The system MUST allow navigation through the list without altering any image decision.

#### Scenario: Move to next and previous image
- **WHEN** the user presses `ArrowDown` / `ArrowUp` or `j` / `k`
- **THEN** the current selection changes and no image decision state is modified

### Requirement: Jump to undecided images
The system MUST allow jumping to the next or previous undecided image using `Shift+ArrowDown` / `Shift+ArrowUp` and `Shift+J` / `Shift+K`.

#### Scenario: Jump forward to the next undecided
- **WHEN** the user presses `Shift+ArrowDown` or `Shift+J`
- **THEN** the selection moves to the nearest later image with an undecided state

### Requirement: Open new path
The system MUST allow users to change the current working directory from the sidebar navigation surface while enforcing the root-directory boundary established at launch. `Ctrl+O` MUST toggle this sidebar surface instead of opening a native picker.

#### Scenario: Toggle sidebar navigation
- **WHEN** the user presses `Ctrl+O`
- **THEN** the sidebar file navigation is shown or hidden

#### Scenario: Open a directory from sidebar
- **WHEN** the user selects a directory row and chooses `open`
- **THEN** the current run is updated to the selected child directory and the image list is rebuilt from supported files in that directory

#### Scenario: Tauri root reselection from sidebar header
- **WHEN** the app runs in Tauri mode and the user clicks the sidebar root control and picks a new directory
- **THEN** the active root/current directory is replaced with the selected directory and the sidebar/image projections are rebuilt

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

### Requirement: Keyboard shortcut for undo
The system MUST provide a keyboard shortcut to undo the most recent command.

#### Scenario: Undo with keyboard
- **WHEN** the user presses `u`
- **THEN** the system triggers undo for the most recent command
