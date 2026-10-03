# Changelog

What changed for people using ezcount, newest first. Add to "Unreleased" as you go; `bun run release` dates it.

## Unreleased

### Added

- French, besides English. The app follows the device's language; the Account window has a switch.
- Log a phone in by scanning a QR code: "Connect a device" in the menu of a device that is already logged in. The code works once, for two minutes.
- An Account window, with the password, the recovery key, the language and logging out.
- A profile: a name and a picture, shown on you in all your groups, for the other members too.
- A picture and a description for each group ("Edit group").
- Search the expenses of a group (title, who paid, amount), show only expenses or reimbursements or what involves one person, and sort by date, amount or title.
- An expense can be paid by several people: "Several people…" under "Paid by", with what each one paid; the amount is their total. Devices still on an older version count the whole expense for whoever paid the most, until they update.

- Categories for expenses (restaurants, groceries, transport…), shown in the list, in the search and as a filter, and kept in CSV files.
- A "Stats" tab: what the group spent in all, by category, by person and by month.
- Member history, in the group's menu: who was added or removed, when and by whom (from this version on).
- Archive a group: it leaves your list for an "Archived" section, on your devices only, and can come back.
- Delete a group for everyone. When someone still owes something, every member has to agree first; members on an older version keep the group.

### Changed

- Changes made on other devices no longer move the list while you read it: a "Refresh" button shows them.

- Amounts and dates are written the way the language does (`€1,234.50`, `1 234,50 €`), with smaller cents. Round amounts have no decimals (`€90`).
- Exporting a group on a computer saves the file in the Downloads folder and says so, with a button to show it.
- One menu on every screen size, with a three-way theme switch (system, light, dark).
- Expenses are listed by day, and a group shows your balance first. On wide screens the group stays beside its expenses.
- Calmer look: fewer greys and font weights, flat secondary buttons, roomier menus.
- "Who are you in this group?" is changed from the group's menu.

### Fixed

- A window taller than the screen (the Account window on a small one) scrolls instead of being cut off.

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
