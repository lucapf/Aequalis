package io.lucapf.tally

import android.os.Bundle
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

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // The core first: it must know the device before the first request.
        Core.start(applicationContext)
        enableEdgeToEdge()
        setContent {
            TallyTheme {
                Surface(Modifier.fillMaxSize()) { TallyApp() }
            }
        }
    }
}

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
    val reply = remember(input) { if (input.isBlank()) null else Core.evaluate(input) }
    // While a word is being typed (no space after it, no Enter), only a result shows: `Rie` on the way
    // to `Rieti to Rome` is no mistake. The same rule as the TUI and the Omarchy widget.
    val finished = submitted || input.endsWith(" ")
    val before = input.take(edit.cursor)
    val offered = remember(before) { Core.suggestions(before) }
    val change = { e: Edit -> edit = e; submitted = false }

    Column(Modifier.fillMaxSize()) {
        Column(Modifier.weight(1f).statusBarsPadding().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            // The top bar: the name, and Settings at the right.
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Tally", style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f))
                TextButton(onClick = { settings = true }, modifier = Modifier.semantics { contentDescription = "Settings" }) {
                    Text("⚙", style = MaterialTheme.typography.titleLarge, modifier = Modifier.clearAndSetSemantics {})
                }
            }
            RequestLine(edit, onCursor = { edit = edit.copy(cursor = it) })
            // A live region: a screen reader says the result, the hint or the error as it changes.
            Column(
                Modifier.weight(1f).verticalScroll(rememberScrollState()).semantics { liveRegion = LiveRegionMode.Polite },
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                when {
                    reply == null -> {}
                    reply.result != null -> Text(reply.result, style = MaterialTheme.typography.headlineMedium)
                    !finished -> {}
                    // What comes next (`now the length unit to convert to: in, ft, yd…`): a hint, in grey.
                    reply.hint != null -> Text(reply.hint, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    reply.choices.isNotEmpty() -> {
                        Text(reply.question ?: "Which one?", style = MaterialTheme.typography.titleMedium)
                        // Each choice is a complete request; the first is the default (⏎).
                        reply.choices.forEach { choice ->
                            OutlinedButton(onClick = { edit = Edit(choice.request, choice.request.length); submitted = true }, modifier = Modifier.fillMaxWidth()) {
                                Text(choice.label)
                            }
                        }
                    }
                    else -> Text(reply.error ?: "", color = MaterialTheme.colorScheme.error)
                }
            }
            // The words offered for the word at the cursor: one tap writes it, then a space.
            LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(bottom = 4.dp)) {
                items(offered.items) { s ->
                    SuggestionChip(onClick = { change(edit.word(s.write, offered.start)) }, label = { Text(s.label) })
                }
            }
        }
        // The keyboard, or a panel in its place.
        when (val open = panel) {
            null -> TallyKeyboard(mode, onKey = { key ->
                when (key) {
                    is Key.Write -> change(edit.write(key.text))
                    Key.Backspace -> change(edit.backspace())
                    Key.Clear -> change(Edit())
                    // On a question, the default choice (the first: the capitals for `Rome Paris`).
                    Key.Enter -> {
                        reply?.choices?.firstOrNull()?.let { edit = Edit(it.request, it.request.length) }
                        submitted = true
                    }
                    is Key.Open -> panel = key.panel
                    is Key.Switch -> mode = key.mode
                }
            })
            else -> PanelView(open, write = { change(edit.word(it)); panel = null }, close = { panel = null }, openLink = { page = it })
        }
    }
}

/** The request, with its cursor: drawn here, not a text field, so the phone's keyboard never
 *  opens; a tap puts the cursor where it lands. */
@Composable
fun RequestLine(edit: Edit, onCursor: (Int) -> Unit) {
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val colors = MaterialTheme.colorScheme
    val blink by rememberInfiniteTransition(label = "cursor").animateFloat(
        initialValue = 1f, targetValue = 0f, animationSpec = infiniteRepeatable(tween(530), RepeatMode.Reverse), label = "blink",
    )
    Box(
        Modifier.fillMaxWidth().border(1.dp, colors.outline, RoundedCornerShape(8.dp)).padding(horizontal = 16.dp, vertical = 14.dp)
            .pointerInput(edit.text) { detectTapGestures { at -> layout?.let { onCursor(it.getOffsetForPosition(at)) } } }
            // What a screen reader says: the request (not the placeholder's example).
            .clearAndSetSemantics { contentDescription = if (edit.text.isEmpty()) "Request, empty" else "Request: ${edit.text}" },
    ) {
        if (edit.text.isEmpty()) Text("10 km mi, 12+4/2, now Tokyo…", color = colors.onSurfaceVariant, style = MaterialTheme.typography.headlineSmall)
        Text(
            edit.text,
            style = MaterialTheme.typography.headlineSmall,
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
