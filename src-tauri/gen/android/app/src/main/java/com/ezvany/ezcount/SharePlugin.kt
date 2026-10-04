package com.ezvany.ezcount

import android.app.Activity
import android.content.Intent
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class ShareArgs {
  lateinit var text: String
  var title: String? = null
}

@InvokeArg
class ShareFileArgs {
  lateinit var path: String
  lateinit var mime: String
  var title: String? = null
}

// Opens Android's share sheet with some text, or with a file from the app's cache folder. Registered from Rust (src-tauri/src/share.rs).
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

  @Command
  fun shareFile(invoke: Invoke) {
    val args = invoke.parseArgs(ShareFileArgs::class.java)
    // The receiving app reads the file through the provider declared in the manifest.
    val uri = FileProvider.getUriForFile(
      activity,
      "${activity.packageName}.fileprovider",
      File(args.path)
    )
    val send = Intent(Intent.ACTION_SEND).apply {
      type = args.mime
      putExtra(Intent.EXTRA_STREAM, uri)
      args.title?.let { putExtra(Intent.EXTRA_SUBJECT, it) }
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    activity.startActivity(Intent.createChooser(send, args.title))
    invoke.resolve()
  }
}
