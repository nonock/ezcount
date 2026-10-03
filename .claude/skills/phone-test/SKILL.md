---
name: phone-test
description: Build the ezcount Android app and try it on the Android phone connected over USB - install the APK, open invite links, check App Links, take screenshots, reach a local relay. Use when a change needs checking on a real phone, or the user asks to test on their phone.
---

# Test on the connected Android phone

Needs the Android toolchain from docs/development.md's "Android toolchain" section, a phone with USB
debugging on, and Windows Developer Mode. Run the commands below in PowerShell from the repo
root.

## 1. Environment

The toolchain paths are user environment variables. A shell started before they were set
doesn't have them (the build then fails with "Java not found"), so load them first:

```powershell
foreach ($n in "JAVA_HOME","ANDROID_HOME","NDK_HOME","GRADLE_USER_HOME") {
  Set-Item "env:$n" ([Environment]::GetEnvironmentVariable($n, "User"))
}
$adb = "$env:ANDROID_HOME\platform-tools\adb.exe"
& $adb devices -l
```

Always call the SDK's `adb` by its full path. Another, older `adb` may be on `PATH`; mixing
versions restarts the adb server, which drops the phone's connection and every
`adb reverse` forward. If `devices` is empty, ask the user to replug the phone and accept the
debugging prompt.

## 2. Build and install

```powershell
# Test build: skips whole-program optimization, several times faster, somewhat larger APK.
$env:CARGO_PROFILE_RELEASE_LTO = "false"; $env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "16"
bun run android:apk
& $adb install -r src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release.apk
```

- Local builds are signed with the public test key; CI builds with the private release key (docs/development.md, "Signing key"). `install -r` keeps the app's data only when the installed app has the same key: otherwise `adb install` fails with `INSTALL_FAILED_UPDATE_INCOMPATIBLE`, and switching means `adb uninstall com.ezvany.ezcount` (ask the user first: it deletes the app's local data; logging in brings the account back). To build with the release key, set `$env:EZCOUNT_RELEASE_KEYSTORE` to the `.p12` file and `$env:EZCOUNT_RELEASE_KEYSTORE_PASSWORD` from its password file (never print the password).
- App Links (step 4) only verify for release-key builds; test-key builds open invite links through the relay's `/join` page.
- The build uses the official relay by default. For another relay, the login screen's
  **Change** link picks one (see step 5 for a local relay).
- The deep-link plugin rewrites `src-tauri/gen/android/app/src/main/AndroidManifest.xml` on
  each build; commit changes to it with the feature that caused them.

## 3. Look at the phone and drive it

```powershell
& $adb shell am start -n com.ezvany.ezcount/.MainActivity   # open the app
& $adb exec-out screencap -p > "$env:TEMP\phone.png"          # then Read the PNG
& $adb shell input tap 673 1357                              # device pixels
& $adb shell input keyevent KEYCODE_BACK
```

Screenshots are shown scaled down: multiply the coordinates you read off them by the scale
factor given with the image before tapping.

**If the screenshot shows the lock screen, stop and ask the user to unlock the phone.** Never
try to unlock it. Anything that needs a person anyway (the camera pointed at a QR code, the
share sheet, typing a password) is also the user's part: give them a short numbered list of
what to do and what they should see.

## 4. Links

Invite links (`https://<relay>/join#…`) and `ezcount://join?…` links open the app. Test both
a running app and a cold start (`& $adb shell am force-stop com.ezvany.ezcount` first). Quote
the URL: `#` and `&` must reach the phone's shell intact.

```powershell
& $adb shell "am start -W -a android.intent.action.VIEW -c android.intent.category.BROWSABLE -d 'https://ezcount-relay.fly.dev/join#v=2&g=GROUP&k=KEY'"
```

Android App Links (`ezcount-relay.fly.dev/join` opening the app without a browser) depend on
the relay's `/.well-known/assetlinks.json` matching the APK's signing key:

```powershell
& $adb shell pm verify-app-links --re-verify com.ezvany.ezcount
& $adb shell pm get-app-links com.ezvany.ezcount   # wait a few seconds; expect "verified"
```

## 5. A relay on this computer

On the phone, `localhost` is the phone itself. Forward it to the computer:

```powershell
bun run relay                              # in another terminal, if not already running
& $adb reverse tcp:8787 tcp:8787
```

Then use `http://localhost:8787` as the server on the phone (login screen → **Change**). The
forward only lasts while the phone stays connected and the adb server keeps running; redo it
after replugging.
