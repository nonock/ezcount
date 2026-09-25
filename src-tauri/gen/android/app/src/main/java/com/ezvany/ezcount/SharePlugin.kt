package com.ezvany.ezcount

import android.app.Activity
import android.content.Intent
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class ShareArgs {
  lateinit var text: String
  var title: String? = null
}

// Opens Android's share sheet with some text. Registered from Rust (src-tauri/src/share.rs).
@TauriPlugin
class SharePlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun shareText(invoke: Invoke) {
    val args = invoke.parseArgs(ShareArgs::class.java)
    val send = Intent(Intent.ACTION_SEND).apply {
      type = "text/plain"
      putExtra(Intent.EXTRA_TEXT, args.text)
      args.title?.let { putExtra(Intent.EXTRA_SUBJECT, it) }
    }
    activity.startActivity(Intent.createChooser(send, args.title))
    invoke.resolve()
  }
}
