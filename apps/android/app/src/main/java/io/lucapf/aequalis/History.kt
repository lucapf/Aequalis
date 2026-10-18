package io.lucapf.aequalis

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.text.format.DateUtils
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.io.File
import java.util.concurrent.Executors

/** A request done with, its result then, when (milliseconds since 1970), and the result's value,
 *  as the core gave it for Copy (`6.2137` of `6.2137 mi`). */
data class HistoryEntry(val request: String, val result: String, val time: Long, val value: String = result)

/** The most entries kept, the oldest dropped: as the TUI's history (`max_history`, src/config.rs). */
const val MAX_HISTORY = 500

/** `entry` first in `entries` (the newest first): an older one with the same request goes, so a
 *  request is there once, at its latest; past `cap` entries, the oldest go. */
fun added(entries: List<HistoryEntry>, entry: HistoryEntry, cap: Int = MAX_HISTORY): List<HistoryEntry> =
    (listOf(entry) + entries.filter { it.request != entry.request }).take(cap)

/** The history as a file's text: a line per entry, its time, request, result and value between
 *  tabs; a tab, a line break or a backslash in them written `\t`, `\n`, `\\`. */
fun historyText(entries: List<HistoryEntry>): String =
    entries.joinToString("") { "${it.time}\t${escaped(it.request)}\t${escaped(it.result)}\t${escaped(it.value)}\n" }

/** The entries of a file's text; a line it can't read (a file cut short) is left out. A line of
 *  an older file has no value: its result stands for it. */
fun historyEntries(text: String): List<HistoryEntry> = text.lineSequence().mapNotNull { line ->
    val fields = line.split('\t')
    val time = fields.firstOrNull()?.toLongOrNull()
    if (fields.size !in 3..4 || time == null || fields[1].isEmpty()) return@mapNotNull null
    val result = unescaped(fields[2])
    HistoryEntry(unescaped(fields[1]), result, time, fields.getOrNull(3)?.let(::unescaped) ?: result)
}.toList()

private fun escaped(s: String) = s.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n").replace("\r", "\\r")

private fun unescaped(s: String): String = buildString {
    var i = 0
    while (i < s.length) {
        val c = s[i]
        if (c == '\\' && i + 1 < s.length) {
            append(when (s[i + 1]) { 't' -> '\t'; 'n' -> '\n'; 'r' -> '\r'; else -> s[i + 1] })
            i += 2
        } else {
            append(c)
            i++
        }
    }
}

/**
 * The requests done with, the newest first, kept in the app's files (`files/history.txt`) so they
 * outlive the app. A request goes in with its result once the user is done with it: ⏎ (or a
 * choice), Copy, Share, C, another request recalled, or the app left (`shown`). Changed on the
 * main thread; the file read and written by a thread of its own, in order.
 */
object History {
    /** The entries, for Compose: a change redraws what shows them. */
    var entries by mutableStateOf<List<HistoryEntry>>(emptyList())
        private set

    /** The request on screen and its result, if it has one: kept when the app is left (`MainActivity.onStop`). */
    var shown: HistoryEntry? = null

    private val disk = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())
    private var file: File? = null
    // Until the file is read, nothing is written: it would lose the entries in it.
    private var loaded = false

    /** Reads the file, off the main thread: the first frame doesn't wait for it. Once per process. */
    fun load(context: Context) {
        if (file != null) return
        val f = File(context.filesDir, "history.txt").also { file = it }
        disk.execute {
            val read = runCatching { historyEntries(f.readText()) }.getOrDefault(emptyList())
            main.post {
                // The entries added meanwhile (the oldest first), over the file's.
                entries = entries.reversed().fold(read) { list, e -> added(list, e) }
                loaded = true
                save()
            }
        }
    }

    /** `request` and its `result` (and the result's `value`), as the newest entry. */
    fun add(request: String, result: String, value: String) {
        val r = request.trim()
        if (r.isEmpty() || result.isBlank()) return
        // Already the newest, with the same result: nothing to change (Copy pressed twice).
        if (entries.firstOrNull()?.let { it.request == r && it.result == result } == true) return
        entries = added(entries, HistoryEntry(r, result, System.currentTimeMillis(), value))
        save()
    }

    /** What was on screen, when the app is left. */
    fun keepShown() {
        shown?.let { add(it.request, it.result, it.value) }
    }

    fun remove(entry: HistoryEntry) {
        entries = entries - entry
        save()
    }

    fun clear() {
        entries = emptyList()
        save()
    }

    private fun save() {
        val f = file ?: return
        if (!loaded) return
        val text = historyText(entries)
        disk.execute {
            // A new file, then renamed over the old one: never half written.
            runCatching {
                val next = File(f.parentFile, "${f.name}.new")
                next.writeText(text)
                next.renameTo(f)
            }
        }
    }
}

/** `Just now`, `5 min. ago`, `Yesterday`, `12 Oct`: when, for people, in the phone's language. */
private fun ago(time: Long): String {
    val now = System.currentTimeMillis()
    // Android says `0 min. ago`.
    if (now - time < DateUtils.MINUTE_IN_MILLIS) return "Just now"
    return DateUtils.getRelativeTimeSpanString(time, now, DateUtils.MINUTE_IN_MILLIS, DateUtils.FORMAT_ABBREV_RELATIVE).toString()
}

/** An entry on one line, `12+3 = 15`, the result in the accent colour: the empty screen's Recent,
 *  where the room is short (landscape). A tap puts the request back in the line; ⧉ copies the result's value. */
@Composable
fun RecentRow(entry: HistoryEntry, recall: (String) -> Unit, copy: (String) -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).clickable(onClickLabel = "Edit the request") { recall(entry.request) }.padding(start = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            buildAnnotatedString {
                append(entry.request)
                withStyle(SpanStyle(color = accent)) { append(" = ${entry.result}") }
            },
            style = MaterialTheme.typography.bodyLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        IconButton(onClick = { copy(entry.value) }, modifier = Modifier.size(36.dp)) {
            Icon(CopyIcon, contentDescription = "Copy the result", modifier = Modifier.size(20.dp))
        }
    }
}

/**
 * An entry: the request, then `= result`, then when. A tap puts the request back in the line, to
 * change it; ⧉ copies the result's value; ⋮ offers Share and Remove.
 */
@Composable
fun HistoryRow(entry: HistoryEntry, recall: (String) -> Unit, copy: (String) -> Unit, share: (String) -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(onClickLabel = "Edit the request") { recall(entry.request) }.padding(start = 16.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(entry.request, style = MaterialTheme.typography.bodyLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text("= ${entry.result}", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(ago(entry.time), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        IconButton(onClick = { copy(entry.value) }) { Icon(CopyIcon, contentDescription = "Copy the result") }
        var menu by remember { mutableStateOf(false) }
        Box {
            IconButton(onClick = { menu = true }) { Icon(MoreIcon, contentDescription = "More") }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(text = { Text("Share") }, leadingIcon = { Icon(ShareIcon, contentDescription = null) }, onClick = { menu = false; share("${entry.request} = ${entry.result}") })
                DropdownMenuItem(text = { Text("Remove") }, leadingIcon = { Icon(DeleteIcon, contentDescription = null) }, onClick = { menu = false; History.remove(entry) })
            }
        }
    }
}

/** The History panel: every entry, the newest first, and at the end Clear the history (asked first). */
@Composable
fun HistoryPanel(recall: (String) -> Unit, copy: (String) -> Unit, share: (String) -> Unit) {
    val entries = History.entries
    if (entries.isEmpty()) {
        Text(
            "No history yet. A request is kept here with its result once you're done with it: ⏎, Copy, Share or C.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(16.dp),
        )
        return
    }
    var asking by remember { mutableStateOf(false) }
    LazyColumn(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        items(entries, key = { it.request }) { e -> HistoryRow(e, recall, copy, share) }
        item {
            OutlinedButton(onClick = { asking = true }, modifier = Modifier.fillMaxWidth().padding(16.dp)) {
                Icon(DeleteIcon, contentDescription = null)
                Text("  Clear the history")
            }
        }
    }
    if (asking) {
        AlertDialog(
            onDismissRequest = { asking = false },
            title = { Text("Clear the history?") },
            text = { Text(if (entries.size == 1) "The request and its result are removed." else "All ${entries.size} requests and their results are removed.") },
            confirmButton = { TextButton(onClick = { asking = false; History.clear() }) { Text("Clear") } },
            dismissButton = { TextButton(onClick = { asking = false }) { Text("Cancel") } },
        )
    }
}
