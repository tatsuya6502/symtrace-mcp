## ADDED Requirements

### Requirement: Server status notification type
The system SHALL define a type for the parameters of the `experimental/serverStatus` notification with `health` (string), `quiescent` (boolean) and an optional `message` (string).

#### Scenario: Deserialize rust-analyzer server status
- **WHEN** the notification parameters `{ "health": "ok", "quiescent": false, "message": null }` are deserialized
- **THEN** the result has `quiescent` false and no message

#### Scenario: Unknown health value
- **WHEN** the parameters carry a `health` value other than `ok`, `warning` or `error`
- **THEN** deserialization SHALL succeed and keep the value as a string

#### Scenario: Missing quiescent field
- **WHEN** the parameters do not contain `quiescent`
- **THEN** deserialization SHALL fail
