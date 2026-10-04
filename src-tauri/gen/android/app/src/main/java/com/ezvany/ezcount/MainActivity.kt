package com.ezvany.ezcount

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  companion object {
    // Whether the app is on screen: it then shows what changed by itself, and nothing is
    // notified (see Notifier.kt and SyncWorker.kt).
    @Volatile
    var visible = false
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Edge-to-edge draws the app under the status bar, the navigation bar and the keyboard,
    // and the WebView doesn't report those areas reliably. Pad the content by them instead, so
    // nothing hides behind the bars and the page shrinks when the keyboard opens (keeping the
    // focused field visible on the login screen).
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val area = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or
          WindowInsetsCompat.Type.displayCutout() or
          WindowInsetsCompat.Type.ime()
      )
      view.setPadding(area.left, area.top, area.right, area.bottom)
      WindowInsetsCompat.CONSUMED
    }

    // What the other members do is notified, from a sync that also runs without the app.
    SyncWorker.schedule(applicationContext)
    askToNotify()
  }

  override fun onResume() {
    super.onResume()
    visible = true
  }

  override fun onPause() {
    visible = false
    super.onPause()
  }

  // Android 13 and later ask the user; before, notifications are allowed from the start.
  private fun askToNotify() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
    val permission = Manifest.permission.POST_NOTIFICATIONS
    if (ContextCompat.checkSelfPermission(this, permission) != PackageManager.PERMISSION_GRANTED) {
      ActivityCompat.requestPermissions(this, arrayOf(permission), 1)
    }
  }
}
