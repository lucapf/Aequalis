package io.lucapf.tally

import android.os.Bundle
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.content.res.Configuration
import android.os.Build
import android.widget.Toast
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
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
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.dp
import io.lucapf.tally.ui.theme.TallyTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // The core first: it must know the device before the first request.
        Core.start(applicationContext)
        // The data shipped with the app, in the background: requests work meanwhile, with what the
        // core already has.
        Thread { Core.seedShipped(applicationContext) }.start()
        enableEdgeToEdge()
        setContent {
            TallyTheme {
                Surface(Modifier.fillMaxSize()) { TallyApp() }
            }
        }
    }
}

/** The core's reply, and the request it answers. */
data class Answer(val request: String, val reply: Reply)

/** The request being written, and the cursor's place in it (an index in the text). Each key makes
 *  a new one; Compose redraws the screen from it. */
data class Edit(val text: String = "", val cursor: Int = 0) {
    fun write(s: String) = Edit(text.take(cursor) + s + text.drop(cursor), cursor + s.length)

    /** The character before the cursor removed (one character, even an emoji's two UTF-16 units). */
    fun backspace(): Edit {
        if (cursor == 0) return this
        val from = text.offsetByCodePoints(cursor, -1)
        return Edit(text.take(from) + text.drop(cursor), from)
    }

    /** A word from a suggestion or a panel, in place of `text[start, cursor)`: apart from what is
     *  before it, and a space after it, so the next word follows. */
    fun word(w: String, start: Int = cursor): Edit {
        val before = text.take(start)
        val gap = if (before.isEmpty() || before.endsWith(" ")) "" else " "
        val head = before + gap + w + " "
        return Edit(head + text.drop(cursor).trimStart(), head.length)
    }
}

/** How an `Edit` is kept when Android recreates the screen (rotation, dark mode, font size, the app
 *  stopped in the background): its text and cursor, as two values the system can store. */
val EditSaver = listSaver<Edit, Any>(save = { listOf(it.text, it.cursor) }, restore = { Edit(it[0] as String, it[1] as Int) })

@Composable
fun TallyApp() {
    // `rememberSaveable`: the user's work survives Android recreating the screen.
    var edit by rememberSaveable(stateSaver = EditSaver) { mutableStateOf(Edit()) }
    // Enter was pressed on this request: its error shows even in the middle of a word.
    var submitted by rememberSaveable { mutableStateOf(false) }
    var panel by rememberSaveable { mutableStateOf<Panel?>(null) }
    // Letters or the calculator: kept while the app is open, and when the screen turns.
    var mode by rememberSaveable { mutableStateOf(Mode.Text) }
    var settings by rememberSaveable { mutableStateOf(false) }
    // A page opened from the Trivia (Wikipedia), shown inside the app.
    var page by rememberSaveable { mutableStateOf<String?>(null) }
    page?.let { url ->
        WebScreen(url, close = { page = null }, modifier = Modifier.statusBarsPadding())
        return
    }
    if (settings) {
        SettingsScreen(close = { settings = false }, modifier = Modifier.statusBarsPadding())
        return
    }

    val input = edit.text
    // The core works off the main thread (`Dispatchers.Default`), so a slow answer (a city search in
    // the archive) never stops the keys. Until the new answer arrives the previous one stays on
    // screen; a key pressed meanwhile cancels the wait, and only the latest request's answer shows.
    val answer by produceState<Answer?>(null, input) {
        value = if (input.isBlank()) null else Answer(input, withContext(Dispatchers.Default) { Core.evaluate(input) })
    }
    val reply = answer?.reply
    // While a word is being typed (no space after it, no Enter), only a result shows: `Rie` on the way
    // to `Rieti to Rome` is no mistake. The same rule as the TUI and the Omarchy widget.
    val finished = submitted || input.endsWith(" ")
    val before = input.take(edit.cursor)
    val offered by produceState(Suggestions(0, emptyList()), before) {
        value = withContext(Dispatchers.Default) { Core.suggestions(before) }
    }
    val change = { e: Edit -> edit = e; submitted = false }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val clipboard = remember { context.getSystemService(ClipboardManager::class.java) }
    // The clipboard's text at the cursor, on one line (a request is one line).
    val paste = {
        val clip = clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
        clip?.replace(Regex("\\s+"), " ")?.trim()?.takeIf { it.isNotEmpty() }?.let { change(edit.write(it)) }
        Unit
    }
    val copy = { text: String ->
        clipboard.setPrimaryClip(ClipData.newPlainText("Tally", text))
        // Since Android 13 the system itself shows what was copied.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) Toast.makeText(context, "Copied", Toast.LENGTH_SHORT).show()
    }
    val share = { text: String ->
        val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text)
        context.startActivity(Intent.createChooser(send, null))
    }
    val onKey = { key: Key ->
        when (key) {
            is Key.Write -> change(edit.write(key.text))
            Key.Backspace -> change(edit.backspace())
            Key.Clear -> change(Edit())
            // On a question, the default choice (the first: the capitals for `Rome Paris`). The
            // answer on screen may still be the previous request's: this one's, then.
            Key.Enter -> {
                scope.launch {
                    val current = answer?.takeIf { it.request == input }?.reply
                        ?: if (input.isBlank()) null else withContext(Dispatchers.Default) { Core.evaluate(input) }
                    if (edit.text != input) return@launch
                    current?.choices?.firstOrNull()?.let { edit = Edit(it.request, it.request.length) }
                    submitted = true
                }
                Unit
            }
            is Key.Open -> panel = key.panel
            is Key.Switch -> mode = key.mode
            Key.Settings -> settings = true
        }
    }
    val workspace = @Composable { landscape: Boolean, modifier: Modifier ->
        Workspace(
            edit, reply, finished, offered, landscape, modifier,
            onCursor = { edit = edit.copy(cursor = it) },
            onPaste = paste,
            onCopy = copy,
            onShare = { result -> share("${input.trim()} = $result") },
            choose = { request -> edit = Edit(request, request.length); submitted = true },
            suggest = { s -> change(edit.word(s.write, offered.start.coerceAtMost(edit.cursor))) },
            openSettings = { settings = true },
        )
    }
    val showPanel = @Composable { open: Panel, size: Modifier ->
        PanelView(open, write = { change(edit.word(it)); panel = null }, close = { panel = null }, openLink = { page = it }, size = size)
    }

    if (LocalConfiguration.current.orientation == Configuration.ORIENTATION_LANDSCAPE) {
        // Landscape: the toolbar on the left, the request and the letters in the middle, the digits
        // (or a panel) on the right; keys at about their portrait size, the rest for the request.
        // One background: the keys stand out by their own colours.
        Row(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            SideToolbar(mode, onKey)
            Column(Modifier.weight(1f).fillMaxHeight()) {
                workspace(true, Modifier.weight(1f).fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp))
                if (panel == null && mode == Mode.Text) LetterPad(onKey)
            }
            when (val open = panel) {
                null -> NumberPad(mode, onKey, Modifier.align(Alignment.Bottom))
                else -> showPanel(open, Modifier.width(380.dp).fillMaxHeight())
            }
        }
        return
    }
    Column(Modifier.fillMaxSize()) {
        workspace(false, Modifier.weight(1f).statusBarsPadding().padding(horizontal = 16.dp))
        // The keyboard, or a panel in its place.
        when (val open = panel) {
            null -> TallyKeyboard(mode, onKey)
            else -> showPanel(open, Modifier.fillMaxWidth().height(KeyboardHeight))
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
    finished: Boolean,
    offered: Suggestions,
    landscape: Boolean,
    modifier: Modifier,
    onCursor: (Int) -> Unit,
    onPaste: () -> Unit,
    onCopy: (String) -> Unit,
    onShare: (String) -> Unit,
    choose: (String) -> Unit,
    suggest: (Suggestion) -> Unit,
    openSettings: () -> Unit,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(if (landscape) 4.dp else 12.dp)) {
        if (!landscape) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Tally", style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f))
                TextButton(onClick = openSettings, modifier = Modifier.semantics { contentDescription = "Settings" }) {
                    Text("⚙", style = MaterialTheme.typography.titleLarge, modifier = Modifier.clearAndSetSemantics {})
                }
            }
        }
        RequestLine(edit, onCursor, onPaste, compact = landscape)
        // A live region: a screen reader says the result, the hint or the error as it changes.
        Column(
            Modifier.weight(1f).verticalScroll(rememberScrollState()).semantics { liveRegion = LiveRegionMode.Polite },
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            when {
                reply == null -> {}
                reply.result != null -> Column {
                    // Copy and Share beside the result (landscape has no room for a row under it).
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        // A tap or a long press copies it, as the buttons beside it.
                        Text(
                            reply.result,
                            style = if (landscape) MaterialTheme.typography.headlineSmall else MaterialTheme.typography.headlineMedium,
                            modifier = Modifier.weight(1f)
                                .combinedClickable(onClickLabel = "Copy", onClick = { onCopy(reply.result) }, onLongClickLabel = "Copy", onLongClick = { onCopy(reply.result) }),
                        )
                        // With a currency, how old its rates are.
                        reply.ratesAge?.let { Text("rates $it old", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                        TextButton(onClick = { onCopy(reply.result) }) { Text("Copy") }
                        TextButton(onClick = { onShare(reply.result) }) { Text("Share") }
                    }
                }
                !finished -> {}
                // What comes next (`now the length unit to convert to: in, ft, yd…`): a hint, in grey.
                reply.hint != null -> Text(reply.hint, color = MaterialTheme.colorScheme.onSurfaceVariant)
                reply.choices.isNotEmpty() -> {
                    Text(reply.question ?: "Which one?", style = MaterialTheme.typography.titleMedium)
                    // Each choice is a complete request; the first is the default (⏎).
                    reply.choices.forEach { choice ->
                        OutlinedButton(onClick = { choose(choice.request) }, modifier = Modifier.fillMaxWidth()) {
                            Text(choice.label)
                        }
                    }
                }
                else -> Text(reply.error ?: "", color = MaterialTheme.colorScheme.error)
            }
        }
        // The words offered for the word at the cursor: one tap writes it, then a space. The chips can
        // still be the previous word's for a moment: their start, at most the cursor.
        LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(bottom = 4.dp)) {
            items(offered.items) { s ->
                SuggestionChip(onClick = { suggest(s) }, label = { Text(s.label) })
            }
        }
    }
}

/** The request, with its cursor: drawn here, not a text field, so the phone's keyboard never
 *  opens; a tap puts the cursor where it lands, a long press pastes. */
@Composable
fun RequestLine(edit: Edit, onCursor: (Int) -> Unit, onPaste: () -> Unit, compact: Boolean = false) {
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val haptic = LocalHapticFeedback.current
    val textStyle = if (compact) MaterialTheme.typography.titleLarge else MaterialTheme.typography.headlineSmall
    val colors = MaterialTheme.colorScheme
    val blink by rememberInfiniteTransition(label = "cursor").animateFloat(
        initialValue = 1f, targetValue = 0f, animationSpec = infiniteRepeatable(tween(530), RepeatMode.Reverse), label = "blink",
    )
    Box(
        Modifier.fillMaxWidth().border(1.dp, colors.outline, RoundedCornerShape(8.dp)).padding(horizontal = 16.dp, vertical = if (compact) 8.dp else 14.dp)
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
