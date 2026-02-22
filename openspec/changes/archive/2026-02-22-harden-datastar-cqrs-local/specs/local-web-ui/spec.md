## MODIFIED Requirements

### Requirement: Server-driven UI with CQRS
The system MUST accept command requests from the UI and respond with server-driven UI updates. Command endpoints under `/cmd/*` MUST return HTTP 204 for accepted commands, and read-model updates MUST be delivered through the Datastar stream endpoint at `GET /events`.

#### Scenario: Command updates UI
- **WHEN** the user submits a left decision command
- **THEN** the server updates state, responds with HTTP 204, and emits corresponding UI updates via the Datastar stream

#### Scenario: Stream endpoint uses GET
- **WHEN** the UI initializes its Datastar event stream connection
- **THEN** it opens `GET /events` and receives server-sent patch events
