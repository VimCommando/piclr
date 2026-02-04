## ADDED Requirements

### Requirement: Per-image decision state
The system MUST track a decision state per image, including undecided and decided (left or right), for the current run.

#### Scenario: Initial state is undecided
- **WHEN** a new run starts
- **THEN** all images are marked as undecided

### Requirement: Configurable action mapping
The system MUST allow left and right decisions to map to one of the supported actions: delete, keep, move, rename, or metadata edit.

#### Scenario: Left action is configured
- **WHEN** the user configures the left action to move
- **THEN** left decisions result in move actions for affected images

### Requirement: Rename pattern
The system MUST support a rename action that uses a pattern of `prefix-number.ext`, where prefix is user-defined and number auto-increments per run.

#### Scenario: Rename with prefix and sequence
- **WHEN** the user chooses the rename action with prefix `trip-`
- **THEN** the image is renamed to `trip-000001.ext` and subsequent renames increment the number

### Requirement: Move target paths
The system MUST allow move actions to target user-defined paths and MUST default to paths relative to the current directory.

#### Scenario: Move to relative target
- **WHEN** the user configures a move target of `keep/`
- **THEN** the file is moved to a `keep/` subdirectory under the current directory

### Requirement: Queue mode with one action per image
The system MUST support an optional queue mode where each image has at most one queued action, and a new decision replaces any prior queued action for that image.

#### Scenario: Replace a queued action
- **WHEN** queue mode is enabled and the user changes an image decision
- **THEN** the previous queued action for that image is replaced by the new action

### Requirement: Apply actions immediately when queue mode is disabled
The system MUST apply actions immediately when queue mode is disabled.

#### Scenario: Immediate apply
- **WHEN** queue mode is disabled and the user chooses Right
- **THEN** the configured right action is applied at once

### Requirement: Apply all queued actions at end
The system MUST apply all queued actions when the user triggers the apply-at-end operation.

#### Scenario: Apply queued actions
- **WHEN** the user triggers apply-at-end
- **THEN** all queued actions are executed and the queue is cleared

### Requirement: Undo last command
The system MUST support undoing the most recent command, reversing its effect on image decision state and queue/application.

#### Scenario: Undo last decision
- **WHEN** the user triggers undo after a decision
- **THEN** the previous image decision state is restored

### Requirement: Safe delete with confirmation
The system MUST treat delete as a safe delete by default by moving files to a `trash/` subdirectory under the current directory, and MUST require explicit confirmation before any permanent deletion during apply-at-end when destructive deletion is enabled.

#### Scenario: Confirm delete at apply time
- **WHEN** queued delete actions are about to be applied
- **THEN** the UI prompts with a confirmation modal defaulting to No before permanent deletion proceeds

### Requirement: Destructive delete opt-in
The system MUST require explicit configuration to enable permanent deletion.

#### Scenario: Destructive delete disabled by default
- **WHEN** the user has not enabled destructive deletion
- **THEN** delete actions move files to `trash/` and do not permanently delete files
### Requirement: Metadata edit action
The system MUST support a metadata edit action that can read and modify image metadata fields without changing the image format.

#### Scenario: Update metadata field
- **WHEN** the user applies a metadata edit action that changes a field value
- **THEN** the metadata is updated for the image and the file extension remains unchanged
