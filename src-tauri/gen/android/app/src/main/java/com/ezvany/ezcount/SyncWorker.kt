package com.ezvany.ezcount

import android.content.Context
import android.util.Log
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit

// Syncs the groups now and then while nobody looks at the app, and shows what the other
// members did. Android freezes the app soon after it leaves the screen and may end it: this
// job is what still runs then, every 15 minutes at best, when the phone has a network.
class SyncWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
  override fun doWork(): Result {
    // On screen, the app syncs by itself.
    if (MainActivity.visible) return Result.success()
    try {
      // The app's Rust library: loaded already when the app runs in this process.
      System.loadLibrary("ezcount_lib")
      val notices = backgroundSync(applicationContext.dataDir.absolutePath)
      if (!MainActivity.visible) Notifier.post(applicationContext, notices)
    } catch (e: Throwable) {
      Log.w("ezcount", "Background sync failed", e)
    }
    return Result.success()
  }

  // `Java_com_ezvany_ezcount_SyncWorker_backgroundSync` in src-tauri/src/notify.rs: one sync,
  // and what the others did as JSON.
  private external fun backgroundSync(dataDir: String): String

  companion object {
    private const val NAME = "sync"

    // Asked for at each start of the app; one that is planned already is kept.
    fun schedule(context: Context) {
      val request = PeriodicWorkRequestBuilder<SyncWorker>(15, TimeUnit.MINUTES)
        .setConstraints(
          Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()
        )
        .build()
      WorkManager.getInstance(context)
        .enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.KEEP, request)
    }
  }
}
