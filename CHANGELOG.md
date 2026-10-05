# Changelog

What changed for people using ezcount, newest first. Add to "Unreleased" as you go; `bun run release` dates it.

## Unreleased

## 0.5.0 - 2026-10-05

### Added

- Pull a group down from its top to bring it up to date with the other devices.
- A button to go back to the top once you have scrolled far down a page.
- Delete your account, from the Account window: it takes your password and removes the account, your profile and your list of groups from the server and from all your devices, for good. Your groups stay for their other members, without your picture and your IBAN.
- The app says when it is too old: for its server, or for a group or an account that a newer version changed in a way it can't read. It then asks to be updated and leaves that data alone, instead of failing to sync or risking to damage it.
- A privacy policy, on the server (`/privacy`), linked from the login screen and the Account window, and a page on how to delete an account (`/delete-account`).

### Changed

- Long lists of expenses stay quick to scroll: days out of view are only drawn when reached.
- The data file of the very first versions (`ezcount_data.json`, from before the database) is no longer imported at startup. It is left where it is.

### Fixed

- "Total spent" on a group and "Your expenses" no longer count the payments between members: they now match the Stats tab.
- On Android, the status bar and the navigation bar take the app's color, in the light and the dark theme.
- The relay answers a device catching up on a large group in pages of a few megabytes, instead of holding the whole group in memory.
- In French, "Suggest a feature" now says in French that a message is too long.

## 0.4.0 - 2026-10-04

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
- Activity, from the button beside "Add Expense": who added, edited, deleted or restored which expense, and who was added to the group or removed from it, when and by whom (from this version on).
- Repeated expenses: "Repeat" in the expense form adds it again every week, month or year (a rent, a subscription). "Repeated expenses", under the same button, lists them and stops them.
- Income: money that came in for the group (a deposit given back, a refund), shared like an expense. Devices still on an older version don't see it, until they update.
- "Transfer" in the expense form records money one member gave another.
- A trash: a deleted expense can be put back, right away with "Undo" or later from "Trash" under that button, by any member. It stays there 30 days.
- An IBAN in your profile, shown to the members of your groups. "Settle Up" then offers a QR code next to what someone owes you: scanned with a banking app, it fills in the transfer (in euros; the IBAN can be copied in any currency).
- The group list says what you owe and what you are owed, in each group and over all of them.
- "Split by item…" in the expense form: enter the lines of a receipt and who each was for, and everyone owes what they took.
- Comments under an expense, from its menu; the activity lists them.
- "Export as PDF" in the group's menu: its balances, who should pay whom and its expenses, on a page to print or send.
- A computer can't scan, so it can now show the code instead: "Connect with your phone" on the login screen (scan it from "Connect a device" on a phone that is logged in), and "Receive from a phone" when joining a group (scan it from the group's "Invite" window with "Send to a computer").
- Notifications on Android when another member adds an expense, a payment or a comment while the app isn't on screen. The phone looks for news every 15 minutes at best, so they can come a while after.
- "Suggest a feature" in the menu: write an idea or a problem, with an e-mail address if you would like an answer. It goes to whoever runs your server.
- Archive a group: it leaves your list for an "Archived" section, on your devices only, and can come back.
- Delete a group for everyone. When someone still owes something, every member has to agree first; members on an older version keep the group.

### Changed

- An invite link opened before logging in or creating an account now joins the group right after, without asking again, and is kept if the app is closed in between (for a week).
- Changes made on other devices no longer move the list while you read it: a "Refresh" button shows them.
- Deleting an expense no longer asks first, since it can be undone.
- The expense form shows the currency as its sign (€, $) in a narrower field, leaving the amount more room.

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
