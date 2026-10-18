package io.lucapf.aequalis

import android.app.ActivityManager
import android.content.Context
import android.os.Debug
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import java.util.Locale

/**
 * The app's memory now, in bytes, as Android counts it for its process: PSS, the pages it uses, its
 * share of those shared with other apps included. By kind: Kotlin's objects (the Java heap), the
 * native heap (the Rust core's and Android's drawing), graphics (the GPU's buffers), code (the
 * app's, Android's and the libraries'), the rest (stacks, the system's). `core`: what the Rust core
 * itself holds, counted by its allocator, and its peak. Then the phone's memory: free and total.
 */
data class MemoryUse(
    val total: Long,
    val kotlin: Long,
    val native: Long,
    val graphics: Long,
    val code: Long,
    val other: Long,
    val core: Pair<Long, Long>?,
    val phoneFree: Long,
    val phoneTotal: Long,
)

/** Counts it now: some milliseconds, off the main thread. */
fun memoryUse(context: Context): MemoryUse {
    val info = Debug.MemoryInfo().also { Debug.getMemoryInfo(it) }
    // Android's summary, in KB, as `adb shell dumpsys meminfo` gives it.
    fun stat(name: String) = (info.getMemoryStat("summary.$name")?.toLongOrNull() ?: 0L) * 1024
    val total = stat("total-pss")
    val kotlin = stat("java-heap")
    val native = stat("native-heap")
    val graphics = stat("graphics")
    val code = stat("code")
    val phone = ActivityManager.MemoryInfo().also { context.getSystemService(ActivityManager::class.java).getMemoryInfo(it) }
    return MemoryUse(
        total, kotlin, native, graphics, code,
        other = (total - kotlin - native - graphics - code).coerceAtLeast(0),
        core = Core.memory(),
        phoneFree = phone.availMem,
        phoneTotal = phone.totalMem,
    )
}

/** `58.2 MB`, `812 KB`: in the phone's number format. */
fun bytes(n: Long): String = when {
    n >= 1_000_000_000 -> String.format(Locale.getDefault(), "%.1f GB", n / 1e9)
    n >= 100_000_000 -> String.format(Locale.getDefault(), "%.0f MB", n / 1e6)
    n >= 1_000_000 -> String.format(Locale.getDefault(), "%.1f MB", n / 1e6)
    else -> String.format(Locale.getDefault(), "%.0f KB", n / 1e3)
}

/**
 * The memory the app uses, under its name (a tap on "Aequalis" shows it, another or a tap here hides
 * it): the total, a bar of its kinds with their sizes, the Rust core's share, the phone's free
 * memory. Counted again every 2 seconds while it's shown.
 */
@Composable
fun MemoryCard(close: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val use by produceState<MemoryUse?>(null) {
        while (true) {
            value = withContext(Dispatchers.Default) { memoryUse(context) }
            delay(2000)
        }
    }
    val colors = MaterialTheme.colorScheme
    Surface(
        color = colors.surfaceContainerHigh,
        shape = RoundedCornerShape(12.dp),
        modifier = modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable(onClickLabel = "Hide the memory use", onClick = close),
    ) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            val u = use
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(MemoryIcon, contentDescription = null, tint = colors.primary, modifier = Modifier.padding(end = 8.dp).size(20.dp))
                Text(if (u == null) "Memory: counting…" else "Memory: ${bytes(u.total)}", style = MaterialTheme.typography.titleSmall)
            }
            if (u == null) return@Column
            // The kinds, as a bar of their shares and a legend.
            val kinds = listOf(
                Triple("Kotlin", u.kotlin, colors.primary),
                Triple("Native", u.native, colors.tertiary),
                Triple("Graphics", u.graphics, colors.secondary),
                Triple("Code", u.code, colors.outline),
                Triple("Other", u.other, colors.outlineVariant),
            ).filter { it.second > 0 }
            Row(Modifier.fillMaxWidth().height(8.dp).clip(RoundedCornerShape(4.dp))) {
                kinds.forEach { (_, size, color) -> Box(Modifier.weight(size.toFloat()).height(8.dp).background(color)) }
            }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                kinds.forEach { (name, size, color) -> Legend(color, "$name ${bytes(size)}") }
            }
            u.core?.let { (held, peak) ->
                Text("The Rust core: ${bytes(held)}, at most ${bytes(peak)} (in Native)", style = MaterialTheme.typography.bodySmall)
            }
            Text("The phone: ${bytes(u.phoneFree)} free of ${bytes(u.phoneTotal)}", style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
        }
    }
}

@Composable
private fun Legend(color: Color, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.padding(end = 4.dp).size(8.dp).clip(CircleShape).background(color))
        Text(text, style = MaterialTheme.typography.bodySmall)
    }
}
