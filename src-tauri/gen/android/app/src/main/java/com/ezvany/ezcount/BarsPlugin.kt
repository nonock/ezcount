package com.ezvany.ezcount

import android.app.Activity
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Build
import android.view.View
import androidx.core.view.WindowCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class BarsArgs {
  lateinit var color: String
  var dark: Boolean = false
}

// Colors what shows behind the status and navigation bars: the app is drawn edge to edge with
// its content padded off them (MainActivity), so that is the window's own background.
// Registered from Rust (src-tauri/src/bars.rs).
@TauriPlugin
class BarsPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun setColor(invoke: Invoke) {
    val args = invoke.parseArgs(BarsArgs::class.java)
    val color = try {
      Color.parseColor(args.color)
    } catch (e: IllegalArgumentException) {
      invoke.reject("Not a color: ${args.color}")
      return
    }
    activity.runOnUiThread {
      val window = activity.window
      window.setBackgroundDrawable(ColorDrawable(color))
      activity.findViewById<View>(android.R.id.content).setBackgroundColor(color)
      // No gray veil over the color with three-button navigation.
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        window.isNavigationBarContrastEnforced = false
      }
      val bars = WindowCompat.getInsetsController(window, window.decorView)
      bars.isAppearanceLightStatusBars = !args.dark
      bars.isAppearanceLightNavigationBars = !args.dark
    }
    invoke.resolve()
  }
}
