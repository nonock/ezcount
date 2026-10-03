# Changelog

What changed for people using ezcount, newest first. Add to "Unreleased" as you go; `bun run release` dates it.

## Unreleased

### Added

- French, besides English. The app follows the device's language; the Account window has a switch.
- Log a phone in by scanning a QR code: "Connect a device" in the menu of a device that is already logged in. The code works once, for two minutes.
- An Account window, with the password, the recovery key, the language and logging out.

### Changed

- Amounts and dates are written the way the language does (`€1,234.50`, `1 234,50 €`), with smaller cents.
- One menu on every screen size, with a three-way theme switch (system, light, dark).
- Expenses are listed by day, and a group shows your balance first.
- Calmer look: fewer greys and font weights, flat secondary buttons, roomier menus.
- "Who are you in this group?" is changed from the group's menu.

## 0.3.0 - 2026-10-03

### Added

- Export a group as a CSV file and import one, including from Tricount (`scripts/tricount-to-csv.ts`).
- Split an expense with set amounts, besides parts.
- Expenses paid in another currency, with the day's exchange rate suggested.

## 0.2.0 - 2026-10-03

First release.

- Groups, expenses split in parts, payments, balances and who owes whom.
- Offline edits that merge between devices, synced through an end-to-end encrypted relay.
- Accounts with a password and a recovery key.
- Invites by link and QR code.
- Windows, Linux and Android apps, and a web version.
