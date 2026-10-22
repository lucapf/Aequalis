package io.lucapf.aequalis

import android.os.Bundle
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.content.res.Configuration
import android.os.Build
import android.widget.Toast
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.VerticalDivider
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.dp
import io.lucapf.aequalis.ui.theme.Appearance
import io.lucapf.aequalis.ui.theme.AequalisTheme
import io.lucapf.aequalis.ui.theme.GREEN
import io.lucapf.aequalis.ui.theme.IVORY
import io.lucapf.aequalis.ui.theme.Skin
import io.lucapf.aequalis.ui.theme.isDark
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : ComponentActivity() {
    companion object {
        /** A request to open with (from the small window's "Open in Aequalis", `QuickActivity`). */
        const val REQUEST = "io.lucapf.aequalis.REQUEST"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // The core in the background (requests wait for it, a few milliseconds), then the data
        // shipped with the app: the first frame needs neither.
        Core.startInBackground(applicationContext)
        // The mode and skin chosen in Settings (the app's own preferences): the first frame's colours.
        Appearance.load(applicationContext)
        // The history, read in the background: the empty screen shows its latest entries.
        History.load(applicationContext)
        // The window started in the splash's green (themes.xml); from now on, as a rotation or a
        // theme change shows it, the app's own background.
        window.setBackgroundDrawableResource(R.color.background)
        enableEdgeToEdge()
        // The splash on a cold start only: not when Android brings back a screen it had stopped.
        val splash = !Splash.played && savedInstanceState == null
        setContent {
            AequalisTheme {
                // Transparent: the theme draws the skin's background behind everything.
                Surface(Modifier.fillMaxSize(), color = Color.Transparent, contentColor = MaterialTheme.colorScheme.onSurface) {
                    AequalisApp(initial = intent.getStringExtra(REQUEST).orEmpty())
                }
            }
            var splashing by remember { mutableStateOf(splash) }
            if (splashing) SplashScreen(done = { Splash.played = true; splashing = false })
        }
    }

    // The app left (not a rotation, which comes back at once): the request on screen, with its
    // result, kept in the history.
    override fun onStop() {
        super.onStop()
        if (!isChangingConfigurations) History.keepShown()
    }
}

/** The core's reply, and the request it answers. */
data class Answer(val request: String, val reply: Reply)

/** The request being written, and the cursor's place in it (an index in the text). Each key makes
 *  a new one; Compose redraws the screen from it. */
data class Edit(val text: String = "", val cursor: Int = 0) {
    /** `s` at the cursor, as much of it as fits in `max` characters (a long paste is cut). */
    fun write(s: String, max: Int): Edit {
        val room = max - text.codePointCount(0, text.length)
        if (room <= 0) return this
        val fits = if (s.codePointCount(0, s.length) <= room) s else s.take(s.offsetByCodePoints(0, room))
        return Edit(text.take(cursor) + fits + text.drop(cursor), cursor + fits.length)
    }

    /** The character before the cursor removed (one character, even an emoji's two UTF-16 units). */
    fun backspace(): Edit {
        if (cursor == 0) return this
        val from = text.offsetByCodePoints(cursor, -1)
        return Edit(text.take(from) + text.drop(cursor), from)
    }

    /** A word from a suggestion or a panel, in place of `text[start, cursor)`: apart from what is
     *  before it, and a space after it, so the next word follows; not written past `max` characters. */
    fun word(w: String, max: Int, start: Int = cursor): Edit {
        val before = text.take(start)
        val gap = if (before.isEmpty() || before.endsWith(" ")) "" else " "
        val head = before + gap + w + " "
        val written = head + text.drop(cursor).trimStart()
        return if (written.codePointCount(0, written.length) > max) this else Edit(written, head.length)
    }
}

/** How an `Edit` is kept when Android recreates the screen (rotation, dark mode, font size, the app
 *  stopped in the background): its text and cursor, as two values the system can store. */
val EditSaver = listSaver<Edit, Any>(save = { listOf(it.text, it.cursor) }, restore = { Edit(it[0] as String, it[1] as Int) })

@Composable
fun AequalisApp(initial: String = "") {
    // `rememberSaveable`: the user's work survives Android recreating the screen.
    var edit by rememberSaveable(stateSaver = EditSaver) { mutableStateOf(Edit(initial, initial.length)) }
    // Enter was pressed on this request: the core then shows its error even in the middle of a word.
    var submitted by rememberSaveable { mutableStateOf(false) }
    var panel by rememberSaveable { mutableStateOf<Panel?>(null) }
    // The Sizes: a strip of keys above the keyboard (the keys stay), not a panel in its place.
    var sizes by rememberSaveable { mutableStateOf(false) }
    // The keyboard collapsed (⌄, a swipe down): its room to the answer, a bar to bring it back.
    var collapsed by rememberSaveable { mutableStateOf(false) }
    // ⏎ on an answer taller than its room collapses the keyboard, once for that request (the bar
    // brings it back and it stays).
    val answerScroll = rememberScrollState()
    var collapsedFor by rememberSaveable { mutableStateOf<String?>(null) }
    LaunchedEffect(submitted, answerScroll.maxValue) {
        if (submitted && answerScroll.maxValue > 0 && collapsedFor != edit.text) {
            collapsedFor = edit.text
            collapsed = true
        }
    }
    // Letters or the calculator: kept while the app is open, and when the screen turns.
    var mode by rememberSaveable { mutableStateOf(Mode.Text) }
    var settings by rememberSaveable { mutableStateOf(false) }
    // About (from Settings): the version, the data, the install links.
    var about by rememberSaveable { mutableStateOf(false) }
    // A page opened from the Trivia (Wikipedia), shown inside the app.
    var page by rememberSaveable { mutableStateOf<String?>(null) }
    page?.let { url ->
        WebScreen(url, close = { page = null }, modifier = Modifier.statusBarsPadding())
        return
    }
    if (about) {
        AboutScreen(close = { about = false }, modifier = Modifier.statusBarsPadding())
        return
    }
    if (settings) {
        SettingsScreen(close = { settings = false }, openAbout = { about = true }, modifier = Modifier.statusBarsPadding())
        return
    }

    val input = edit.text
    // The core works off the main thread (`Dispatchers.Default`), so a slow answer (a city search in
    // the archive) never stops the keys. Until the new answer arrives the previous one stays on
    // screen; a key pressed meanwhile cancels the wait, and only the latest request's answer shows.
    // What it shows (nothing but a result while a word is being typed) the core decides.
    val answer by produceState<Answer?>(null, input, submitted) {
        value = if (input.isBlank()) null else Answer(input, withContext(Dispatchers.Default) { Core.evaluate(input, submitted) })
    }
    val reply = answer?.reply
    // The request on screen and its result, kept when the app is left.
    SideEffect { History.shown = answer?.let { a -> a.reply.result?.let { HistoryEntry(a.request, it, 0, a.reply.value ?: it) } } }
    val before = input.take(edit.cursor)
    val offered by produceState(Suggestions(0, emptyList()), before) {
        value = withContext(Dispatchers.Default) { Core.suggestions(before) }
    }
    val change = { e: Edit -> edit = e; submitted = false }
    val scope = rememberCoroutineScope()
    // A request into the history with its result (none: an error, a question, not kept).
    val add = { request: String, r: Reply? -> r?.result?.let { History.add(request, it, r.value ?: it) }; Unit }
    val keepShown = { answer?.let { add(it.request, it.reply) }; Unit }
    // `request` into the history, once its answer is known (`known`, or the core's, computed now).
    val keep = { request: String, known: Reply? ->
        scope.launch {
            add(request, known ?: if (request.isBlank()) null else withContext(Dispatchers.Default) { Core.evaluate(request, true) })
        }
        Unit
    }
    // An entry of the history back in the line, to change it; the one on screen kept first.
    val recall = { request: String -> keepShown(); edit = Edit(request, request.length); submitted = false; panel = null }
    val context = LocalContext.current
    val clipboard = remember { context.getSystemService(ClipboardManager::class.java) }
    // The clipboard's text at the cursor, on one line (a request is one line).
    val paste = {
        val clip = clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
        clip?.replace(Regex("\\s+"), " ")?.trim()?.takeIf { it.isNotEmpty() }?.let { change(edit.write(it, Core.maxInput)) }
        Unit
    }
    val copy = { text: String ->
        clipboard.setPrimaryClip(ClipData.newPlainText("Aequalis", text))
        // Since Android 13 the system itself shows what was copied.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) Toast.makeText(context, "Copied", Toast.LENGTH_SHORT).show()
    }
    val share = { text: String ->
        val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text)
        context.startActivity(Intent.createChooser(send, null))
    }
    val onKey = { key: Key ->
        when (key) {
            is Key.Write -> change(edit.write(key.text, Core.maxInput))
            Key.Backspace -> change(edit.backspace())
            Key.Clear -> { keepShown(); change(Edit()) }
            // On a question, the core's default choice (the capitals for `Rome Paris`).
            Key.Enter -> {
                scope.launch {
                    val current = if (input.isBlank()) null else withContext(Dispatchers.Default) { Core.evaluate(input, true) }
                    if (edit.text != input) return@launch
                    val chosen = current?.default
                    chosen?.let { edit = Edit(it, it.length) }
                    submitted = true
                    // Done with it: into the history (a choice's result computed first).
                    keep(chosen ?: input, if (chosen == null) current else null)
                }
                Unit
            }
            // The Sizes: their strip shown or hidden. Another panel: opened; its key again closes it.
            is Key.Open -> when (key.panel) {
                Panel.Sizes -> { sizes = !sizes; panel = null }
                else -> panel = if (panel == key.panel) null else key.panel
            }
            is Key.Switch -> mode = key.mode
            Key.Settings -> settings = true
            Key.Collapse -> collapsed = true
        }
    }
    val suggest = { s: Suggestion -> change(edit.word(s.write, Core.maxInput, offered.start.coerceAtMost(edit.cursor))) }
    val workspace = @Composable { landscape: Boolean, modifier: Modifier, recent: Boolean ->
        Workspace(
            edit, reply, offered, landscape, modifier,
            // In landscape the request line is drawn apart, across the whole width.
            request = !landscape,
            onCursor = { edit = edit.copy(cursor = it) },
            onPaste = paste,
            // Only the value (`7.4565`, not `12 km mi = 7.4565 mi`), as the core gives it.
            onCopy = { value -> copy(value); keepShown() },
            onShare = { value -> share(value); keepShown() },
            choose = { request -> edit = Edit(request, request.length); submitted = true; keep(request, null) },
            suggest = suggest,
            openSettings = { settings = true },
            recall = recall,
            copyResult = { value -> copy(value) },
            openHistory = { panel = Panel.History },
            recent = recent,
            scroll = answerScroll,
        )
    }
    // A key of the Sizes: after the request, `shoes` brought along when it has no item word (the core's).
    val sizeKey = { key: String -> Core.writeSizeKey(edit.text, key).let { change(Edit(it, it.length)) } }
    val showPanel = @Composable { open: Panel, size: Modifier, wide: Boolean ->
        PanelView(
            open, write = { change(edit.word(it, Core.maxInput)); panel = null }, close = { panel = null }, openLink = { page = it }, unitClass = reply?.unitClass,
            recall = recall, copy = { value -> copy(value) }, share = share, size = size, wide = wide,
            sizeKey = { sizeKey(it); panel = null },
        )
    }

    // The window's size class (Material's: compact under 600 dp wide): a large window (a foldable
    // open, a tablet), the keys at a phone's size and a panel always beside them.
    val configuration = LocalConfiguration.current
    if (configuration.screenWidthDp >= 600 && configuration.screenHeightDp >= 480) {
        // The panel beside the keys: History at first, the tab chosen kept (folding and unfolding
        // too). Its key on the keyboard picks it; what it writes leaves it open.
        var docked by rememberSaveable { mutableStateOf(Panel.History) }
        val pick = { key: Key -> if (key is Key.Open) docked = key.panel else onKey(key) }
        // Two equal panes: on a foldable they meet at the fold, nothing to tap across it.
        Row(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            Column(Modifier.weight(1f).fillMaxHeight(), horizontalAlignment = Alignment.CenterHorizontally) {
                workspace(false, Modifier.weight(1f).fillMaxWidth().padding(horizontal = 16.dp), false)
                AequalisKeyboard(mode, pick, Modifier.widthIn(max = 480.dp), open = docked)
            }
            VerticalDivider()
            Box(Modifier.weight(1f).fillMaxHeight()) {
                PanelView(
                    docked, write = { change(edit.word(it, Core.maxInput)) }, close = {}, openLink = { page = it }, unitClass = reply?.unitClass,
                    recall = recall, copy = { value -> copy(value) }, share = share, size = Modifier.fillMaxSize(),
                    docked = true, select = { docked = it }, sizeKey = sizeKey,
                )
            }
        }
        return
    }
    if (configuration.orientation == Configuration.ORIENTATION_LANDSCAPE) {
        // Landscape: the toolbar on the left; the request across the rest of the width; under it the
        // answer and the letters, and on the right the digits (Sci above them) or a panel. Keys at
        // about their portrait size; letters and digits both shown, so no 123 mode.
        Row(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            SideToolbar(onKey, open = panel)
            Column(Modifier.weight(1f).fillMaxHeight()) {
                // The request, and the words offered beside it: the result keeps the room under it.
                Row(Modifier.padding(start = 8.dp, top = 6.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    RequestLine(edit, onCursor = { edit = edit.copy(cursor = it) }, onPaste = paste, compact = true, modifier = Modifier.weight(1f))
                    if (offered.items.isNotEmpty()) SuggestionRow(offered, suggest, Modifier.widthIn(max = 320.dp).padding(start = 8.dp))
                }
                when (val open = panel) {
                    // A panel: the whole space under the request, its levels side by side.
                    null -> Row(Modifier.weight(1f)) {
                        Column(Modifier.weight(1f).fillMaxHeight()) {
                            workspace(true, Modifier.weight(1f).fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), true)
                            // The Sizes in the letters' place (the digits stay); collapsed, a bar.
                            when {
                                collapsed -> KeyboardBar { collapsed = false }
                                sizes -> SizesPad(sizeKey, onKey, Modifier.swipe(down = true) { collapsed = true })
                                else -> LetterPad(onKey, Modifier.swipe(down = true) { collapsed = true })
                            }
                        }
                        if (!collapsed) NumberPad(scientific = mode == Mode.Scientific, onKey, Modifier.align(Alignment.Bottom))
                    }
                    else -> showPanel(open, Modifier.weight(1f).fillMaxWidth().padding(top = 6.dp), true)
                }
            }
        }
        return
    }
    // The phone's keyboard (the Currencies' search): the panel over it, the screen above smaller.
    Column(Modifier.fillMaxSize().imePadding()) {
        workspace(false, Modifier.weight(1f).statusBarsPadding().padding(horizontal = 16.dp), true)
        // The keyboard (the Sizes' strip above it, when shown), or a panel in its place.
        when (val open = panel) {
            // The Sizes on: their keys in the letters' place; in the calculator (no letters), a strip above it.
            null -> {
                if (sizes && mode != Mode.Text && !collapsed) SizesStrip(sizeKey)
                if (collapsed) KeyboardBar { collapsed = false } else AequalisKeyboard(mode, onKey, sizes = sizes, sizeKey = sizeKey)
            }
            else -> showPanel(open, Modifier.fillMaxWidth().height(KeyboardHeight), false)
        }
    }
}

/** The request, the answer and the words offered: the screen above (portrait) or beside (landscape)
 *  the keys. In portrait the app's name and ⚙ on top; in landscape (⚙ on the toolbar) all smaller,
 *  the height being short. */
@Composable
fun Workspace(
    edit: Edit,
    reply: Reply?,
    offered: Suggestions,
    landscape: Boolean,
    modifier: Modifier,
    request: Boolean,
    onCursor: (Int) -> Unit,
    onPaste: () -> Unit,
    onCopy: (String) -> Unit,
    onShare: (String) -> Unit,
    choose: (String) -> Unit,
    suggest: (Suggestion) -> Unit,
    openSettings: () -> Unit,
    recall: (String) -> Unit,
    copyResult: (String) -> Unit,
    openHistory: () -> Unit,
    // The latest entries under an empty request: not when the History is open beside it.
    recent: Boolean = true,
    // The answer's scroll: past its room (the main screen's), ⏎ collapses the keyboard.
    scroll: ScrollState = rememberScrollState(),
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(if (landscape) 4.dp else 12.dp)) {
        if (!landscape) {
            // A tap on the name: the memory the app uses, under it; another tap hides it.
            var memory by rememberSaveable { mutableStateOf(false) }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) {
                    // The mark (Splash.kt) for the name: the q ivory on the Aequalis skin (as the
                    // icon), else green, lighter on a dark screen.
                    AequalisMark(
                        q = when {
                            Appearance.skin == Skin.Aequalis -> IVORY
                            isDark() -> Color(0xFF8FD3B0)
                            else -> GREEN
                        },
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .clickable(onClickLabel = if (memory) "Hide the memory use" else "Show the memory use") { memory = !memory }
                            .padding(horizontal = 4.dp, vertical = 4.dp)
                            .size(width = 34.dp, height = 32.dp),
                    )
                }
                IconButton(onClick = openSettings) { Icon(SettingsIcon, contentDescription = "Settings", tint = MaterialTheme.colorScheme.primary) }
            }
            if (memory) MemoryCard(close = { memory = false })
        }
        if (request) RequestLine(edit, onCursor, onPaste, compact = landscape)
        // A live region: a screen reader says the result, the hint or the error as it changes.
        Column(
            Modifier.weight(1f).verticalScroll(scroll).semantics { liveRegion = LiveRegionMode.Polite },
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // A calculation's grouping (`(12 + (4 / 2))`), as the TUI shows it.
            reply?.grouping?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            when {
                // Nothing written yet: the latest entries of the history, to use again.
                edit.text.isEmpty() -> if (recent) Recent(recall, copyResult, openHistory)
                reply == null -> {}
                // What shows, the core decides: nothing but a result while a word is being typed.
                else -> when (reply.show) {
                    Show.Result -> Column {
                        val result = reply.result.orEmpty()
                        val value = reply.value ?: result
                        val whole = reply.expression ?: result
                        // Copy and Share beside the result (landscape has no room for a row under it).
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            // A tap copies the value, a long press the whole expression.
                            val copying = Modifier.weight(1f)
                                .combinedClickable(onClickLabel = "Copy the value", onClick = { onCopy(value) }, onLongClickLabel = "Copy the expression", onLongClick = { onCopy(whole) })
                            // A table (`shoes 42`): a line per person, from the extra results.
                            if ('\n' in result && reply.extra.isNotEmpty()) PeopleSizes(reply.extra, copying)
                            else Text(result, style = resultStyle(result, landscape), modifier = copying)
                            // With a currency, how old its rates are.
                            reply.rates?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                            CopyMenu(value, whole, onCopy)
                            IconButton(onClick = { onShare(whole) }) { Icon(ShareIcon, contentDescription = "Share") }
                        }
                        // The other results (the shoe sizes beside a currency), then the notes and warnings.
                        Extras(reply)
                        reply.details.forEach { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                    }
                    // What comes next (`now the length unit to convert to: in, ft, yd…`): a hint, in grey;
                    // with the results it already has (`42 eu`: the shoe sizes).
                    Show.Hint -> Column {
                        Text(reply.message, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Extras(reply)
                    }
                    Show.Question -> {
                        Text(reply.message, style = MaterialTheme.typography.titleMedium)
                        // Each choice is a complete request; the default one is ⏎'s.
                        reply.choices.forEach { choice ->
                            OutlinedButton(onClick = { choose(choice.request) }, modifier = Modifier.fillMaxWidth()) {
                                Text(choice.label)
                            }
                        }
                    }
                    Show.Error -> Text(reply.message, color = MaterialTheme.colorScheme.error)
                    Show.None -> {}
                }
            }
        }
        // Landscape: beside the request line instead, the short room above the keys all the result's.
        if (!landscape) SuggestionRow(offered, suggest, Modifier.padding(bottom = 4.dp))
    }
}

/** A result's style: large; a table (`42 eu shoes`, several lines) smaller and in a monospace
 *  font, so that its columns line up. */
@Composable
fun resultStyle(result: String, landscape: Boolean): TextStyle = when {
    '\n' in result -> MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace)
    landscape -> MaterialTheme.typography.headlineSmall
    else -> MaterialTheme.typography.headlineMedium
}

/** Every person's sizes (`shoes 42`), a line each: the person in bold, then their sizes, wrapping
 *  under themselves: `Men  US 8.5 · UK 7.5 · EU 42 · JP 26.5 · CN 42`. */
@Composable
fun PeopleSizes(people: List<Extra>, modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        people.forEach { p ->
            Row {
                Text(
                    p.type.removePrefix("shoes ").replaceFirstChar { it.uppercase() },
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                    modifier = Modifier.width(76.dp),
                )
                Text(p.results.joinToString(" · "), style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
            }
        }
    }
}

/** The other results under the result or the hint, a line each: beside a currency
 *  `shoes: US 8.5 men, US 10 women`; beside a size (`US 8.5 men`) the other people's, as they are.
 *  A table says them already: none. */
@Composable
fun Extras(reply: Reply) {
    val result = reply.result.orEmpty()
    if ('\n' in result) return
    val size = result.substringBefore(' ') in listOf("US", "UK", "EU", "JP", "CN")
    reply.extra.forEach { e ->
        val line = e.results.joinToString(", ")
        Text(if (size) line else "${e.type}: $line", style = MaterialTheme.typography.titleMedium)
    }
}

/** The words offered for the word at the cursor: one tap writes it, then a space. The chips can
 *  still be the previous word's for a moment: their start, at most the cursor. */
@Composable
fun SuggestionRow(offered: Suggestions, suggest: (Suggestion) -> Unit, modifier: Modifier = Modifier) {
    LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = modifier) {
        items(offered.items) { s ->
            SuggestionChip(onClick = { suggest(s) }, label = { Text(s.label) })
        }
    }
}

/** Copy: a menu of the two ways, each as it will be copied: the value (`3.27`), the whole
 *  expression (`12 AED = 3.27 USD`). */
@Composable
private fun CopyMenu(value: String, whole: String, copy: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) { Icon(CopyIcon, contentDescription = "Copy") }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            listOf("Value" to value, "Expression" to whole).distinctBy { it.second }.forEach { (label, text) ->
                DropdownMenuItem(
                    text = {
                        Column {
                            Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Text(text, style = MaterialTheme.typography.bodyLarge)
                        }
                    },
                    onClick = { open = false; copy(text) },
                )
            }
        }
    }
}

/** The three latest entries of the history, under "Recent" and All (the History panel); nothing
 *  while the history is empty. */
@Composable
private fun Recent(recall: (String) -> Unit, copy: (String) -> Unit, openHistory: () -> Unit) {
    val latest = History.entries.take(3)
    if (latest.isEmpty()) return
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Recent", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f).padding(start = 4.dp))
            // Small: the room above the keys is short in landscape.
            Text(
                "All",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.clip(RoundedCornerShape(8.dp)).clickable(onClickLabel = "Open the history", onClick = openHistory).padding(horizontal = 12.dp, vertical = 4.dp),
            )
        }
        latest.forEach { RecentRow(it, recall, copy) }
    }
}

/** The request, with its cursor: drawn here, not a text field, so the phone's keyboard never
 *  opens; a tap puts the cursor where it lands, a long press pastes. */
@Composable
fun RequestLine(edit: Edit, onCursor: (Int) -> Unit, onPaste: () -> Unit, compact: Boolean = false, modifier: Modifier = Modifier) {
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val haptic = LocalHapticFeedback.current
    val textStyle = if (compact) MaterialTheme.typography.titleLarge else MaterialTheme.typography.headlineSmall
    val colors = MaterialTheme.colorScheme
    val blink by rememberInfiniteTransition(label = "cursor").animateFloat(
        initialValue = 1f, targetValue = 0f, animationSpec = infiniteRepeatable(tween(530), RepeatMode.Reverse), label = "blink",
    )
    Box(
        modifier.fillMaxWidth().border(1.dp, colors.outline, RoundedCornerShape(8.dp)).padding(horizontal = 16.dp, vertical = if (compact) 8.dp else 14.dp)
            .pointerInput(edit, onPaste) {
                detectTapGestures(
                    onTap = { at -> layout?.let { onCursor(it.getOffsetForPosition(at)) } },
                    onLongPress = { haptic.performHapticFeedback(HapticFeedbackType.LongPress); onPaste() },
                )
            }
            // What a screen reader says: the request (not the placeholder's example), and Paste.
            .clearAndSetSemantics {
                contentDescription = if (edit.text.isEmpty()) "Request, empty" else "Request: ${edit.text}"
                customActions = listOf(CustomAccessibilityAction("Paste") { onPaste(); true })
            },
    ) {
        if (edit.text.isEmpty()) Text("10 km mi, 12+4/2, now Tokyo…", color = colors.onSurfaceVariant, style = textStyle)
        Text(
            edit.text,
            style = textStyle,
            onTextLayout = { layout = it },
            // The cursor: a thin bar where the text's layout puts it, blinking.
            modifier = Modifier.fillMaxWidth().drawWithContent {
                drawContent()
                layout?.let { l ->
                    val rect = l.getCursorRect(edit.cursor.coerceIn(0, l.layoutInput.text.length))
                    drawRect(colors.primary.copy(alpha = blink), topLeft = rect.topLeft, size = Size(2.dp.toPx(), rect.height))
                }
            },
        )
    }
}
