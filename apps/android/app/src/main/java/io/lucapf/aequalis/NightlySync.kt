package io.lucapf.aequalis

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.time.Duration
import java.time.ZonedDateTime
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The online data downloaded at night, without the app open: once a day, on an unmetered network
 * (Wi-Fi) and with the battery not low. Android chooses the moment: about [HOUR]:[MINUTE], later
 * when the phone is busy, off the network or saving battery. The core adds every day missed since
 * the last download (the rates of past days: `history::sync`), so a night skipped costs nothing.
 */
object NightlySync {
    /** The setting that switches it (src/settings.rs). */
    const val SETTING = "android.nightly_sync"
    private const val NAME = "nightly-sync"
    private const val HOUR = 2
    private const val MINUTE = 30
    /** Failed attempts (no network answering…) before it waits for the next night. */
    private const val ATTEMPTS = 3

    /** Schedules the download, or cancels it. Scheduling twice keeps the first, so it does not
     *  move the hour of a night already planned. */
    fun apply(context: Context, enabled: Boolean) {
        val work = WorkManager.getInstance(context.applicationContext)
        if (!enabled) {
            work.cancelUniqueWork(NAME)
            return
        }
        val request = PeriodicWorkRequestBuilder<Worker>(1, TimeUnit.DAYS)
            .setInitialDelay(untilNight(ZonedDateTime.now()))
            .setConstraints(constraints())
            .build()
        work.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.KEEP, request)
    }

    /** Wi-Fi (any network that is not metered) and a battery that is not low. */
    fun constraints(): Constraints = Constraints.Builder()
        .setRequiredNetworkType(NetworkType.UNMETERED)
        .setRequiresBatteryNotLow(true)
        .build()

    /** How long from `now` to the next [HOUR]:[MINUTE] (today's if it is still ahead). */
    fun untilNight(now: ZonedDateTime): Duration {
        var night = now.withHour(HOUR).withMinute(MINUTE).withSecond(0).withNano(0)
        if (!night.isAfter(now)) night = night.plusDays(1)
        return Duration.between(now, night)
    }

    /** What WorkManager runs: the core starts if the app is not running, and downloads what is due. */
    class Worker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
        override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
            Core.startNow(applicationContext)
            val synced = Core.download(auto = true)
            if (synced.failed && runAttemptCount < ATTEMPTS) Result.retry() else Result.success()
        }
    }
}
