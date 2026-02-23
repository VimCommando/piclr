## ADDED Requirements

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
The system MUST present one primary image at a time for review, while optionally showing partial previews of adjacent images to communicate next/previous navigation.

#### Scenario: Viewing the current image
- **WHEN** the user is in the viewing state
- **THEN** the UI displays one whole currently selected image and may show partial previous/next images as stack context

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
- **WHEN** the user presses Down to move forward or Up to move backward
- **THEN** the current selection changes and no image decision state is modified

### Requirement: Jump to undecided images
The system MUST allow jumping to the next or previous undecided image using Shift+Down and Shift+Up.

#### Scenario: Jump forward to the next undecided
- **WHEN** the user presses Shift+Down
- **THEN** the selection moves to the nearest later image with an undecided state

### Requirement: Open new path
The system MUST allow the user to change directory from the sidebar navigation surface. `Ctrl+O` MUST toggle the sidebar. In web-only mode, the launch directory remains fixed as root. In Tauri mode, the sidebar root control MAY select a new root directory.

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
The system MUST apply the left or right action to the currently selected image when the user presses the corresponding arrow key or click zone.

#### Scenario: Left decision applied
- **WHEN** the user presses Left or clicks the left zone
- **THEN** the configured left action is applied to the current image and the UI advances according to navigation rules
