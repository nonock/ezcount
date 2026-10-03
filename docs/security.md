# Invites, accounts and what the relay can see

## Inviting people

Open the group and tap **Invite**. It shows an invite link, `https://<relay>/join#v=2&g=<group>&k=<key>`, and its QR code:

- **Share** (Android) sends the link through any app with the system share sheet; **Copy Link** works everywhere.
- **Opening the link** on a phone with ezcount installed opens the app with the invite filled in. For the Fly.io relay, Android opens it directly (App Links, declared in `tauri.conf.json` and verified against the relay's `assetlinks.json`). Otherwise the relay's `/join` page shows an **Open in ezcount** button (an `ezcount://join?…` link, also registered on Windows and Linux) and the invite to paste by hand, plus **Open in your browser** when the relay serves the web version.
- **Scanning:** in **Join with Code**, Android offers **Scan QR Code**, which joins right away. A phone's own camera app also reads the code and opens the link.
- The newcomer then picks who they are in the group. An invite link opened before logging in waits for the login.

The key sits after the `#`, which browsers never send to a server, so the relay never sees it, not even when the `/join` page is opened. Invites in the older `ezcount://join?…` form keep working.

**Anyone with a group's invite link can read and edit that group.**

## Groups are end-to-end encrypted

The secret in the invite link never leaves the devices (`core/src/crypto.rs`). Two keys are derived from it with HKDF-SHA256:

- an auth token, the only thing sent to the relay (which stores its hash),
- an encryption key, used with XChaCha20-Poly1305 to seal every update, with the group ID bound in.

The relay operator can see group IDs, update sizes and timing, but not names, amounts or titles. Any change the relay makes to an update is detected.

## Accounts are too

The password never leaves the device (`CredentialKeys` in `crypto.rs`): Argon2id (64 MiB, 3 passes) stretches it into a login token, whose hash the relay stores, and a key that encrypts your random account key. The relay keeps that encrypted blob and returns it on login. It can check logins but can't read your account or your groups. Consequences:

- Nobody can reset a forgotten password, so sign-up hands out a **recovery key** (160 random bits, shown once as `XXXX-XXXX-…` in Crockford base32). It works like a second password: it derives its own token, whose hash the relay stores, and its own key, which encrypts a second copy of the account key. **Forgot password?** on the login screen takes the username, the recovery key and a new password. Each recovery key works once: using it sets a new one, shown right away. Without the password and the recovery key, the account and its group list are lost (group members can still re-invite you).
- The account window changes the password (other devices stay logged in: only the copy of the account key that the password unlocks changes) and makes a new recovery key, which also gives accounts from before recovery keys their first one.
- Someone who steals the relay's database can try to guess passwords offline, at Argon2's cost per guess. So sign-up refuses guessable passwords: [zxcvbn](https://github.com/shssoichiro/zxcvbn-rs) must rate it 3 of 4 or more (over 10⁸ guesses), counting common passwords, words, keyboard patterns, dates and the username. The form shows the rating as you type. Online guessing is limited per username: 5 failed attempts per 15 minutes from one network, 50 from all of them.
- Logging out removes the account's data from the device. It refuses while edits aren't uploaded yet, unless you confirm.

## Logging a phone in with a QR code

**Connect a device**, in the menu of a device that is already logged in, asks for the password and shows a QR code that the login screen of a phone scans. The code works once, for two minutes: the account's key waits on the relay, in memory, encrypted with a secret that only the QR code carries (`sync-server/src/links.rs`, `LinkKeys` in `crypto.rs`), so the relay can't read it. Anyone who photographs the code within those two minutes, before the phone uses it, gets the account.

## Transport

The relay speaks plain HTTP behind a TLS reverse proxy. The app accepts `http://` only for a relay on the same device or a private network (home network, emulator, Tailscale), where tokens stay local, and requires `https://` anywhere else. It also never follows redirects, which could send tokens elsewhere.

## The web version trusts the server

A website's code comes from the server on every visit, so whoever controls the relay could serve code that reads your data. The apps don't have that risk. The relay serves the page under a strict Content-Security-Policy (scripts from itself only, requests to itself only).

## Limitations

Removing a member doesn't revoke their access: a removed member who kept the invite link can still read and edit the group. Likewise, logging out of a lost device doesn't lock it out. Both need key rotation.
