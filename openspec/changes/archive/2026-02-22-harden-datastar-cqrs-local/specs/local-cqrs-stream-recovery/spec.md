## ADDED Requirements

### Requirement: Stream resync after subscriber lag
The system MUST restore a subscriber to a consistent read model when the `/events` stream subscriber lags and misses broadcasted incremental updates.

#### Scenario: Lagged subscriber receives full snapshot patches
- **WHEN** a connected subscriber encounters stream lag and one or more incremental events are dropped
- **THEN** the server emits a full read-model patch sequence for that subscriber before continuing incremental streaming

### Requirement: Reconnect bootstrap from authoritative state
The system MUST send a full read-model bootstrap patch sequence when a subscriber opens or reopens the `/events` stream.

#### Scenario: Initial stream open emits full view
- **WHEN** the UI opens `/events`
- **THEN** the server emits a full set of patches derived from authoritative server state before emitting incremental updates
