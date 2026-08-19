# Deployment

## Financial Invariants

The campaign contract maintains several financial invariants to ensure the integrity of the system. These invariants are checked at various points in the code to prevent inconsistencies and potential exploits.

- **`raised_amount`**: This is the total amount of funds raised by the campaign. It is the sum of all donations made to the campaign.
- **`released_amount`**: This is the total amount of funds that have been released to the campaign owner. It is the sum of all milestone releases.
- **`total_donated`**: This is the total amount of funds donated by a specific donor.
- **`raised_per_asset`**: This is the total amount of funds raised for a specific asset.

The following invariants must always hold true:

- `raised_amount` >= `released_amount`
- `raised_amount` = sum of all `raised_per_asset`
- `total_donated` for a donor = sum of all `per_asset` donations for that donor

## Rounding Strategy

The campaign contract uses integer arithmetic for all financial calculations. This means that there is no floating-point arithmetic and therefore no rounding errors. When calculating the amount to release for each asset in a milestone, the contract uses a simple division and multiplication strategy. The last asset in the list of accepted assets will receive the remainder of the release amount, ensuring that the full amount is released.
