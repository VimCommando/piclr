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
The system MUST present exactly one image at a time to the user for review.

#### Scenario: Viewing the current image
- **WHEN** the user is in the viewing state
- **THEN** the UI displays the currently selected image and its position in the list

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
The system MUST allow the user to open a new directory from the UI via Ctrl+O.

#### Scenario: Open a new directory
- **WHEN** the user presses Ctrl+O and selects a new directory
- **THEN** the current run is replaced with a scan of the selected directory

### Requirement: Apply decision to current image
The system MUST apply the left or right action to the currently selected image when the user presses the corresponding directional key bindings or clicks the corresponding half of the stack view.

#### Scenario: Left decision applied
- **WHEN** the user presses `ArrowLeft` or `h`, or clicks the left half of the stack view
- **THEN** the configured left action is applied to the current image and the UI advances according to navigation rules

#### Scenario: Right decision applied
- **WHEN** the user presses `ArrowRight` or `l`, or clicks the right half of the stack view
- **THEN** the configured right action is applied to the current image and the UI advances according to navigation rules

### Requirement: Keyboard shortcuts for queue, list, help, and close
The system MUST provide keyboard shortcuts for opening queue/list/help views and closing the top modal.

#### Scenario: Open queue, image list, and help
- **WHEN** the user presses `q`, `i`, or `?`
- **THEN** the system opens the queue, image list, or help modal respectively

#### Scenario: Close top modal
- **WHEN** the user presses `Escape`
- **THEN** the system closes the top modal in the modal stack

### Requirement: Keyboard shortcut for undo
The system MUST provide a keyboard shortcut to undo the most recent command.

#### Scenario: Undo with keyboard
- **WHEN** the user presses `u`
- **THEN** the system triggers undo for the most recent command
