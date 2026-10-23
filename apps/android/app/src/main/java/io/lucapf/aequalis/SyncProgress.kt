package io.lucapf.aequalis

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.InputStream
import java.io.OutputStream

/** A sync by hand, as Settings shows it: whether it runs, how far its big download is (`null`: no
 *  percentage, nothing big downloading), and the download speed in bytes a second. Written by
 *  `Core.fetch` in the sync's thread; read by the Sync button and the line under it. */
object SyncProgress {
    var active by mutableStateOf(false)
    var fraction by mutableStateOf<Float?>(null)
    var speed by mutableLongStateOf(0L)

    /** A download from this size shows its percentage; smaller ones (the rates, the live values)
     *  finish too fast to show and leave the bar as it is. */
    private const val BIG = 200_000L

    fun start() {
        fraction = null
        speed = 0L
        active = true
    }

    fun stop() {
        active = false
    }

    /** `input` into `output`, the speed (the last second's bytes) and, for a big download of
     *  `length` bytes (`-1`: unknown), the percentage said at most every 100 ms while a sync by hand
     *  runs. */
    fun copy(input: InputStream, output: OutputStream, length: Long) {
        val buffer = ByteArray(64 * 1024)
        val window = ArrayDeque<Pair<Long, Long>>()
        var total = 0L
        var said = 0L
        while (true) {
            val n = input.read(buffer)
            if (n < 0) break
            output.write(buffer, 0, n)
            total += n
            if (!active) continue
            val now = System.nanoTime()
            window.addLast(now to total)
            while (window.size > 1 && now - window.first().first > 1_000_000_000L) window.removeFirst()
            if (now - said >= 100_000_000L) {
                said = now
                val (since, from) = window.first()
                if (now > since) speed = (total - from) * 1_000_000_000L / (now - since)
                if (length >= BIG) fraction = (total.toFloat() / length).coerceIn(0f, 1f)
            }
        }
        // The end, whatever the last time said: a fast download is said whole.
        if (active && length >= BIG) fraction = (total.toFloat() / length).coerceIn(0f, 1f)
    }
}

/** A download speed for people, as browsers write it (1000-based): `800 B/s`, `400 kB/s`, `1.2 MB/s`. */
fun speedText(bytesPerSecond: Long): String = when {
    bytesPerSecond < 1_000 -> "$bytesPerSecond B/s"
    bytesPerSecond < 1_000_000 -> "${bytesPerSecond / 1_000} kB/s"
    else -> "%.1f MB/s".format(java.util.Locale.ROOT, bytesPerSecond / 1_000_000.0)
}
