# Changelog

All notable changes to the common crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2024-01-15

### Added

- Initial shared types: `CampaignStatus`, `MilestoneStatus`, `AssetInfo`, `ErrorCode`
- Version constant `VERSION` and `version()` function
- Utility functions:
  - `validate_address()` - Validates address is not zero address
  - `validate_future_timestamp()` - Validates timestamp is in future
  - `validate_positive_amount()` - Validates amount is positive
  - `current_timestamp()` - Returns current ledger timestamp
  - `is_campaign_ended()` - Checks if campaign has ended
  - `assets_equal()` - Compares two asset infos
  - `is_asset_accepted()` - Checks if asset is in accepted list
  - `description_hash_bytes()` - Returns description hash as byte array
- Unit tests for all utility functions

### Versioning Policy

- `VERSION` constant is bumped on any breaking change
- Breaking changes require a minor version bump
- Non-breaking additions require a patch version bump
