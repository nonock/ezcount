package com.ezvany.ezcount

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class PostArgs {
  lateinit var notices: String
}

// What the app's own sync found while it runs, to show as notifications when the app isn't
// on screen. Registered from Rust (src-tauri/src/notify.rs).
@TauriPlugin
class NotifyPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun post(invoke: Invoke) {
    val args = invoke.parseArgs(PostArgs::class.java)
    // On screen, the app shows what changed by itself.
    if (!MainActivity.visible) Notifier.post(activity.applicationContext, args.notices)
    invoke.resolve()
  }
}
