package com.ezvany.ezcount

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import java.text.NumberFormat
import java.util.Currency
import org.json.JSONArray
import org.json.JSONObject

// Shows what the other members did in the user's groups, from the notices the Rust side
// found while syncing (`Notice` in core/src/notices.rs), in the phone's language.
object Notifier {
  private const val CHANNEL = "activity"

  // Past this many in one group, one notification says how many there are.
  private const val MOST_PER_GROUP = 3

  fun post(context: Context, json: String) {
    val manager = NotificationManagerCompat.from(context)
    // Refused by the user, or not granted yet: nothing is shown.
    if (!manager.areNotificationsEnabled()) return
    val notices = try {
      val list = JSONArray(json)
      (0 until list.length()).map { list.getJSONObject(it) }
    } catch (e: Exception) {
      return
    }
    if (notices.isEmpty()) return

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      val channel = NotificationChannel(
        CHANNEL,
        context.getString(R.string.notify_channel),
        NotificationManager.IMPORTANCE_DEFAULT
      )
      context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }
    val open = PendingIntent.getActivity(
      context,
      0,
      Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
      PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
    )

    for ((groupId, inGroup) in notices.groupBy { it.getString("group_id") }) {
      val group = inGroup.first().getString("group")
      val texts = if (inGroup.size > MOST_PER_GROUP) {
        listOf(context.getString(R.string.notify_many, inGroup.size))
      } else {
        inGroup.map { sentence(context, it) }
      }
      texts.forEachIndexed { index, text ->
        val notification = NotificationCompat.Builder(context, CHANNEL)
          .setSmallIcon(R.drawable.ic_notification)
          .setContentTitle(group)
          .setContentText(text)
          .setStyle(NotificationCompat.BigTextStyle().bigText(text))
          .setContentIntent(open)
          .setAutoCancel(true)
          .build()
        try {
          // One per thing said, so a later one doesn't take the place of an unread one.
          manager.notify("$groupId/$text/$index".hashCode(), notification)
        } catch (e: SecurityException) {
          // The permission went between the check and here.
        }
      }
    }
  }

  private fun sentence(context: Context, notice: JSONObject): String {
    val by = if (notice.isNull("by")) null else notice.getString("by")
    val text = if (notice.isNull("text")) null else notice.getString("text")
    val title = notice.getString("title")
    val amount = money(notice.getLong("amount_cents"), notice.getString("currency"))
    return when (notice.getString("kind")) {
      "comment" ->
        if (by != null) context.getString(R.string.notify_comment_by, by, title, text ?: "")
        else context.getString(R.string.notify_comment, title, text ?: "")
      "payment" ->
        if (by != null && text != null) context.getString(R.string.notify_payment, by, text, amount)
        else context.getString(R.string.notify_expense, title, amount)
      "income" ->
        if (by != null) context.getString(R.string.notify_income_by, by, title, amount)
        else context.getString(R.string.notify_income, title, amount)
      else ->
        if (by != null) context.getString(R.string.notify_expense_by, by, title, amount)
        else context.getString(R.string.notify_expense, title, amount)
    }
  }

  private fun money(cents: Long, currency: String): String {
    return try {
      val format = NumberFormat.getCurrencyInstance()
      format.currency = Currency.getInstance(currency)
      format.format(cents / 100.0)
    } catch (e: Exception) {
      // A code Android doesn't know.
      String.format("%.2f %s", cents / 100.0, currency)
    }
  }
}
