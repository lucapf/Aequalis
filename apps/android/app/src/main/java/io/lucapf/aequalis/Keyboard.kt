package io.lucapf.aequalis

import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.ceil
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext

/** The keyboard's height: the panels (Measures, Currencies, Trivia, History) take its place exactly. */
val KeyboardHeight: Dp = 380.dp

/** What a key does. */
sealed interface Key {
    /** Writes this text. */
    data class Write(val text: String) : Key
    data object Backspace : Key
    /** The request cleared (the ✕ in the request box). */
    data object Clear : Key
    data object Enter : Key
    /** Opens a panel over the keyboard. */
    data class Open(val panel: Panel) : Key
    /** Switches the keyboard: letters, or the calculator (basic or scientific). */
    data class Switch(val mode: Mode) : Key
    /** Opens the Settings (the landscape toolbar; in portrait, ⚙ at the top). */
    data object Settings : Key
    /** Collapses the keyboard (its ⌄ key, a swipe down), its room to the answer; a bar brings it back. */
    data object Collapse : Key
    /** Brings the collapsed keyboard back (the landscape toolbar's key, lit while it is collapsed). */
    data object Expand : Key
}

enum class Panel {
    Measures, Currencies, Sizes, Trivia, History;

    /** Its icon: on its key, and beside its title. */
    val icon: ImageVector
        get() = when (this) {
            Measures -> MeasuresIcon
            Currencies -> CurrenciesIcon
            Sizes -> SizesIcon
            Trivia -> TriviaIcon
            History -> HistoryIcon
        }
}

/** The keyboard's layouts: letters (with the digits and operators), or the calculator's keys. */
enum class Mode { Text, Basic, Scientific }

/**
 * Aequalis's own keyboard, in one of its `mode`s. Letters: as on a phone, with ⇧ for the capitals
 * (what is written is matched in any case, but for the units whose case matters, `MB` and `Mb`,
 * also offered by the suggestions and the Measures), with the digits and the operators above
 * them. The calculator: the digits and operators in a grid, and in scientific mode the functions
 * and constants beside them.
 */
@Composable
fun AequalisKeyboard(
    mode: Mode,
    onKey: (Key) -> Unit,
    modifier: Modifier = Modifier,
    open: Panel? = null,
    // The Sizes on: their keys in the letters' place (the digits and operators stay).
    sizes: Boolean = false,
    sizeKey: (String) -> Unit = {},
    // Shift's state, kept by the screen (not by this keyboard) so that a rotation or a fold keeps it.
    capitals: Capitals = remember { Capitals() },
) {
    // `open`: a panel docked beside the keyboard (a large screen), whose tabs replace the panel keys.
    // A swipe down collapses it (beside a docked panel it stays).
    Surface(color = MaterialTheme.colorScheme.surfaceContainer, modifier = if (open == null) modifier.swipe(down = true) { onKey(Key.Collapse) } else modifier) {
        Column(
            Modifier.fillMaxWidth().navigationBarsPadding().height(KeyboardHeight).padding(4.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            // The Sizes on (their keys in the letters' place): what they are and the way back, over the
            // toolbar and its Sizes key. In the calculator the strip above the keyboard says it.
            if (sizes && mode == Mode.Text) SizesHint(Modifier.padding(horizontal = 4.dp))
            // The toolbar, in every mode: the keyboards and the panels, apart from the keys typed.
            KeyRow {
                if (open == null) KeyButton("⌄", weight = 0.7f, tone = Tone.Action) { onKey(Key.Collapse) }
                switches(mode).forEach { (label, to) -> KeyButton(label, weight = 1.2f, tone = Tone.Action, small = true) { onKey(Key.Switch(to)) } }
                // Beside a docked panel its tabs choose: no keys for them here.
                if (open == null) PanelKeys(onKey, sizes) else Spacer(Modifier.weight(4f))
            }
            when (mode) {
                Mode.Text -> Letters(onKey, if (sizes) sizeKey else null, capitals)
                Mode.Basic, Mode.Scientific -> Calculator(scientific = mode == Mode.Scientific, onKey)
            }
        }
    }
}

/** A vertical swipe (down, or up) of more than 48 dp on this: `then`. Taps on what is under it go on
 *  as before: a drag starts only past the touch slop. */
fun Modifier.swipe(down: Boolean, then: () -> Unit): Modifier = pointerInput(down) {
    var moved = 0f
    detectVerticalDragGestures(onDragStart = { moved = 0f }, onDragEnd = {
        if ((if (down) moved else -moved) > 48.dp.toPx()) then()
    }) { _, dy -> moved += dy }
}

/** The keyboard collapsed: a bar where it was, its room given to the answer; a tap or a swipe up
 *  brings the keys back. */
@Composable
fun KeyboardBar(show: () -> Unit) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth().swipe(down = false, then = show)) {
        Row(
            Modifier.fillMaxWidth().navigationBarsPadding().clickable(onClickLabel = "Show the keyboard", onClick = show).padding(vertical = 10.dp),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(KeyboardUpIcon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            Text("keyboard", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** The toolbar's keys that switch the keyboard: to the calculator or back to the letters, and in
 *  the calculator between basic and scientific. */
private fun switches(mode: Mode): List<Pair<String, Mode>> = when (mode) {
    Mode.Text -> listOf("123" to Mode.Basic)
    Mode.Basic -> listOf("ABC" to Mode.Text, "Sci" to Mode.Scientific)
    Mode.Scientific -> listOf("ABC" to Mode.Text, "Basic" to Mode.Basic)
}

/** The operators as people write them, written as Aequalis reads them: × is *, ÷ is /, − is -. */
private val OPERATORS = listOf("+" to "+", "−" to "-", "×" to "*", "÷" to "/", "^" to "^", "(" to "(", ")" to ")", "%" to "%", "." to ".", ":" to ":")

@Composable
private fun ColumnScope.Letters(onKey: (Key) -> Unit, sizeKey: ((String) -> Unit)?, capitals: Capitals) {
    val write = { s: String -> onKey(Key.Write(s)) }
    // Above the digits, next to them: the request's numbers and operators are typed together.
    KeyRow { OPERATORS.forEach { (shown, written) -> KeyButton(shown, tone = Tone.Operator) { write(written) } } }
    KeyRow { "1234567890".forEach { c -> CharKey(c, write) } }
    // The Sizes on: their keys where the letters are.
    if (sizeKey != null) {
        return SizeKeys(sizeKey, onKey) { content -> KeyRow(content) }
    }
    KeyRow { "qwertyuiop".forEach { c -> LetterKey(c, capitals, write) } }
    // The middle row is half a key shorter on each side, as on a phone; the last one has ⇧ and ⌫.
    KeyRow {
        Spacer(Modifier.weight(0.5f))
        "asdfghjkl".forEach { c -> LetterKey(c, capitals, write) }
        Spacer(Modifier.weight(0.5f))
    }
    KeyRow {
        ShiftKey(capitals)
        "zxcvbnm".forEach { c -> LetterKey(c, capitals, write) }
        BackspaceKey(onKey, weight = 1.5f)
    }
    // Space and ⏎ alone, under the thumbs. No comma: coordinates take a space (`41.9 12.5`), and the
    // suggestions write the names that need one (`Springfield, Illinois`).
    KeyRow {
        KeyButton("space", weight = 4f, small = true) { write(" ") }
        KeyButton("⏎", weight = 1.4f, tone = Tone.Enter) { onKey(Key.Enter) }
    }
}

/** A calculator key: its label, and what it writes (null: ⌫ or ⏎, by its label). */
private data class Calc(val label: String, val writes: String?, val tone: Tone)

private fun digit(d: String) = Calc(d, d, Tone.Letter)
private fun op(shown: String, written: String) = Calc(shown, written, Tone.Operator)

/** The calculator's grid: digits and operators; ⌫ and ⏎ in the last row. */
private val CALCULATOR = listOf(
    listOf(op("(", "("), op(")", ")"), op("%", "%"), op("÷", "/")),
    listOf(digit("7"), digit("8"), digit("9"), op("×", "*")),
    listOf(digit("4"), digit("5"), digit("6"), op("−", "-")),
    listOf(digit("1"), digit("2"), digit("3"), op("+", "+")),
    listOf(digit("0"), digit("."), Calc("⌫", null, Tone.Action), Calc("⏎", null, Tone.Enter)),
)

/** Scientific, landscape: the functions in four columns of four keys, beside the columns of the keys that
 *  were the first two rows (`MOVED_FUNCTIONS`, `MOVED_OPERATORS`). Each writes its bracket (`sin(`),
 *  `)` closes it; ° makes the angle degrees (`sin(30°)`); `x!` is written after a number, `x⁻¹` opens
 *  `1/(`, `10ˣ` writes `10^` and `EE` the exponent of scientific notation (`2E5` is 2 × 10^5), and
 *  `ˣ√` writes `root(`, the number, then `,` and the index: `root(27, 3)`. */
private val SCIENCE_LANDSCAPE = listOf(
    listOf("cos" to "cos(", "tan" to "tan(", "√" to "sqrt(", "∛" to "cbrt("),
    listOf("cosh" to "cosh(", "tanh" to "tanh(", "ˣ√" to "root(", "," to ", "),
    listOf("log" to "log(", "x!" to "!", "x⁻¹" to "1/(", "^" to "^"),
    listOf("e" to "e", "°" to "°", "10ˣ" to "10^", "EE" to "E"),
)

/** The functions of the first row, in a column of their own beside the digits. */
private val MOVED_FUNCTIONS = listOf("sin" to "sin(", "sinh" to "sinh(", "ln" to "ln(", "π" to "pi")

/** The operators of the first row of the digits, in a column of their own on their right. */
private val MOVED_OPERATORS = listOf(op("÷", "/"), op("%", "%"), op("(", "("), op(")", ")"))

/** Scientific, portrait: one column less, to fit the screen: no `,` (the root is `^(1/` here: the
 *  number, `ˣ√`, the index and `)`, as `27^(1/3)`), no `°`, `EE`, `x⁻¹` or `e` (`10ˣ` writes the
 *  powers of ten, `1/(` and `e` are typed). */
private val SCIENCE_PORTRAIT = listOf(
    listOf("sin" to "sin(", "cos" to "cos(", "tan" to "tan(", "√" to "sqrt(", "∛" to "cbrt("),
    listOf("sinh" to "sinh(", "cosh" to "cosh(", "tanh" to "tanh(", "ˣ√" to "^(1/", "^" to "^"),
    listOf("ln" to "ln(", "log" to "log(", "x!" to "!", "π" to "pi", "10ˣ" to "10^"),
)

/** The functions' rows in a column: as many as the grid has. */
private const val SCIENCE_ROWS = 5

/** The narrowest a function key gets before the columns scroll. */
private val FunctionKeyWidth = 40.dp

/** The functions: every column of `science` side by side as wide as fits, the rest scrolling. */
@Composable
private fun FunctionKeys(science: List<List<Pair<String, String>>>, write: (String) -> Unit, modifier: Modifier = Modifier) {
    BoxWithConstraints(modifier) {
        // Not all of them fit: the last column showing is cut in half, which says that the rest
        // scrolls.
        val fit = (maxWidth / FunctionKeyWidth).toInt().coerceAtLeast(1)
        val shown = if (fit >= science.size) science.size.toFloat() else fit + 0.5f
        val width = (maxWidth - 4.dp * (ceil(shown) - 1)) / shown
        Row(Modifier.fillMaxSize().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            science.forEach { column ->
                Column(Modifier.width(width).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    column.forEach { (label, written) -> ColumnKey(label, small = true) { write(written) } }
                    repeat(SCIENCE_ROWS - column.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

/** The calculator: digits and operators in a grid; scientific: the functions on the left. */
@Composable
private fun ColumnScope.Calculator(scientific: Boolean, onKey: (Key) -> Unit) {
    if (!scientific) return CALCULATOR.forEach { row -> KeyRow { row.forEach { k -> CalcKey(k, onKey) } } }
    Row(Modifier.fillMaxWidth().weight(CALCULATOR.size.toFloat()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        FunctionKeys(SCIENCE_PORTRAIT, { onKey(Key.Write(it)) }, Modifier.weight(SCIENCE_PORTRAIT.size.toFloat()).fillMaxHeight())
        Column(Modifier.weight(4f).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            CALCULATOR.forEach { row -> KeyRow { row.forEach { k -> CalcKey(k, onKey) } } }
        }
    }
}

/** A key's height in landscape: about the portrait one, so the request and the result keep the
 *  rest of the screen. */
private val LandscapeRow: Dp = 44.dp

/** Landscape, on the left: the toolbar, as a column (the keyboard's collapse and Sci, the panels, Settings). */
@Composable
fun SideToolbar(onKey: (Key) -> Unit, mode: Mode, collapsed: Boolean, open: Panel? = null, sizes: Boolean = false, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxHeight().width(88.dp).padding(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        // The keyboard collapsed: its key lit, with the arrow up, and a tap brings the keys back.
        when {
            collapsed -> ColumnKey("⌃", lit = true) { onKey(Key.Expand) }
            else -> ColumnKey("⌄") { onKey(Key.Collapse) }
        }
        // The scientific keys: lit while they show, a tap again goes back to the letters (as the Sizes').
        val scientific = mode == Mode.Scientific
        ColumnKey("Sci", small = true, lit = scientific) { onKey(Key.Switch(if (scientific) Mode.Text else Mode.Scientific)) }
        // The panel open: its key in the accent colour (the panel has no title bar here); tapped
        // again, it closes the panel. The Sizes, on the keys, the same.
        Panel.entries.forEach { p ->
            // The Sizes on: what their keys are and the way back, over their key.
            if (p == Panel.Sizes && sizes) SizesHintTall()
            ColumnKey(p.name, small = true, lit = p == open || (p == Panel.Sizes && sizes)) { onKey(Key.Open(p)) }
        }
        ColumnKey("⚙") { onKey(Key.Settings) }
    }
}

/** Landscape, under the request: the letters, as in portrait, with ⌫, space and ⏎. */
@Composable
fun LetterPad(onKey: (Key) -> Unit, capitals: Capitals, modifier: Modifier = Modifier) {
    val write = { s: String -> onKey(Key.Write(s)) }
    Column(modifier.fillMaxWidth().padding(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        PadRow { "qwertyuiop".forEach { c -> LetterKey(c, capitals, write) } }
        PadRow {
            Spacer(Modifier.weight(0.5f))
            "asdfghjkl".forEach { c -> LetterKey(c, capitals, write) }
            Spacer(Modifier.weight(0.5f))
        }
        PadRow {
            ShiftKey(capitals)
            "zxcvbnm".forEach { c -> LetterKey(c, capitals, write) }
            BackspaceKey(onKey, weight = 1.5f)
        }
        PadRow {
            KeyButton("space", weight = 4f, small = true) { write(" ") }
            KeyButton("⏎", weight = 1.4f, tone = Tone.Enter) { onKey(Key.Enter) }
        }
    }
}

/** The width of a column of keys beside the letters (landscape). */
private val KeyColumnWidth = 44.dp

/** The digits in columns, as the landscape lays them out: four rows (`7 8 9 ×`, `4 5 6 −`, `1 2 3 +`, then
 *  `0 . : ^` beside the letters, which have ⌫ and ⏎, or `0 . ⌫ ⏎` without them); `× − +` in the last column. */
private fun digitColumns(letters: Boolean): List<List<Calc>> {
    val rows = listOf(
        listOf(digit("7"), digit("8"), digit("9"), op("×", "*")),
        listOf(digit("4"), digit("5"), digit("6"), op("−", "-")),
        listOf(digit("1"), digit("2"), digit("3"), op("+", "+")),
        if (letters) listOf(digit("0"), digit("."), op(":", ":"), op("^", "^")) else listOf(digit("0"), digit("."), Calc("⌫", null, Tone.Action), Calc("⏎", null, Tone.Enter)),
    )
    return (0 until 4).map { c -> rows.map { it[c] } }
}

/** A column of the landscape keys, four rows tall (padded as the letters' rows are). */
@Composable
private fun PadColumn(modifier: Modifier, content: @Composable ColumnScope.() -> Unit) =
    Column(modifier.padding(vertical = 4.dp).height(LandscapeRow * 4 + 4.dp * 3), verticalArrangement = Arrangement.spacedBy(4.dp), content = content)

/** A calculator key in a column. */
@Composable
private fun ColumnScope.CalcColumnKey(k: Calc, onKey: (Key) -> Unit) {
    val size = Modifier.weight(1f).fillMaxWidth()
    when {
        k.writes != null -> KeyCap(k.label, size, k.tone, small = false, repeat = false) { onKey(Key.Write(k.writes)) }
        k.label == "⌫" -> KeyCap(k.label, size, Tone.Action, small = false, repeat = true) { onKey(Key.Backspace) }
        else -> KeyCap(k.label, size, k.tone, small = false, repeat = false) { onKey(Key.Enter) }
    }
}

/**
 * Landscape, under the result: the keys in four rows, across the width. With the letters (or the Sizes in
 * their place): the digits, then `÷ % ( )` in a column on their right. Scientific, no letters: the
 * functions in four columns, the first row's (`sin sinh ln π`) in a column beside them, then the digits
 * and `÷ % ( )` on their right.
 */
@Composable
fun LandscapeKeys(mode: Mode, sizes: Boolean, onKey: (Key) -> Unit, sizeKey: (String) -> Unit, capitals: Capitals, modifier: Modifier = Modifier) {
    val write = { s: String -> onKey(Key.Write(s)) }
    val scientific = mode == Mode.Scientific && !sizes
    Row(modifier.fillMaxWidth().padding(horizontal = 4.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.Bottom) {
        // Each group of columns shares the width in the Sci layout; beside the letters, a fixed one.
        val column: (Modifier) -> Modifier = { m -> if (scientific) m.weight(1f) else m.width(KeyColumnWidth) }
        when {
            scientific -> {
                SCIENCE_LANDSCAPE.forEach { keys -> PadColumn(Modifier.weight(1f)) { keys.forEach { (label, written) -> ColumnKey(label, small = true) { write(written) } } } }
                PadColumn(Modifier.weight(1f)) { MOVED_FUNCTIONS.forEach { (label, written) -> ColumnKey(label, small = true) { write(written) } } }
            }
            sizes -> SizesPad(sizeKey, onKey, Modifier.weight(1f))
            else -> LetterPad(onKey, capitals, Modifier.weight(1f))
        }
        digitColumns(letters = !scientific).forEach { keys -> PadColumn(Modifier.let(column)) { keys.forEach { CalcColumnKey(it, onKey) } } }
        PadColumn(Modifier.let(column)) { MOVED_OPERATORS.forEach { CalcColumnKey(it, onKey) } }
    }
}

/** Landscape, the Sizes on: big keys in the letters' place (the digits stay beside them); the hint
 *  is over the Sizes key, on the toolbar (`SideToolbar`). */
@Composable
fun SizesPad(sizeKey: (String) -> Unit, onKey: (Key) -> Unit, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth().padding(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        SizeKeys(sizeKey, onKey) { content -> PadRow(content) }
    }
}

/** The Sizes' keys in four rows, as many as the letters (`row`: the layout's, portrait or
 *  landscape): Shoes, Clothes and the people; the systems; the letter sizes; ABC back to the letters,
 *  space, ⌫ and ⏎. Written by the core (`sizeKey`: a system or a letter alone, a group in place of
 *  the other, a person in place of another). */
@Composable
private fun SizeKeys(sizeKey: (String) -> Unit, onKey: (Key) -> Unit, row: @Composable (@Composable RowScope.() -> Unit) -> Unit) {
    val sizes = produceState<Sizes?>(null) { value = withContext(Dispatchers.Default) { Core.sizes() } }.value
    val systems = sizes?.systems.orEmpty()
    row {
        sizes?.groups.orEmpty().forEach { (name, word) -> KeyButton(name, weight = 1.4f, tone = Tone.Operator, small = true) { sizeKey(word) } }
        sizes?.people.orEmpty().forEach { (name, word) -> KeyButton(name, tone = Tone.Operator, small = true) { sizeKey(word) } }
    }
    row { systems.forEach { (name, code) -> KeyButton(name, tone = Tone.Operator, small = true) { sizeKey(code) } } }
    row { sizes?.letters.orEmpty().forEach { (name, letter) -> KeyButton(name, tone = Tone.Operator) { sizeKey(letter) } } }
    row {
        KeyButton("ABC", weight = 1.2f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Sizes)) }
        KeyButton("space", weight = 2.6f, small = true) { onKey(Key.Write(" ")) }
        BackspaceKey(onKey, weight = 1.2f)
        KeyButton("⏎", weight = 1.2f, tone = Tone.Enter) { onKey(Key.Enter) }
    }
}

/** What the Sizes keys are, and the way back, over them: the Sizes key stays lit while they show. */
@Composable
fun SizesHint(modifier: Modifier = Modifier) =
    Text(SIZES_HINT, color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 11.sp, maxLines = 1, textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = modifier.fillMaxWidth())

private const val SIZES_HINT = "Size keys · tap Sizes again to go back"

/** The same hint in the landscape toolbar's width: a few short lines, over the Sizes key. */
@Composable
private fun SizesHintTall() =
    Text(
        SIZES_HINT,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        fontSize = 9.sp,
        lineHeight = 10.sp,
        maxLines = 4,
        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        modifier = Modifier.fillMaxWidth(),
    )

/** A row of the landscape pads: a fixed height. */
@Composable
private fun PadRow(content: @Composable RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth().height(LandscapeRow), horizontalArrangement = Arrangement.spacedBy(4.dp), content = content)
}

@Composable
private fun RowScope.CalcKey(k: Calc, onKey: (Key) -> Unit) = when {
    k.writes != null -> KeyButton(k.label, tone = k.tone) { onKey(Key.Write(k.writes)) }
    k.label == "⌫" -> BackspaceKey(onKey)
    else -> KeyButton(k.label, tone = k.tone) { onKey(Key.Enter) }
}

/** A key of the landscape toolbar: the column's width, a share of its height. */
@Composable
private fun ColumnScope.ColumnKey(label: String, small: Boolean = false, lit: Boolean = false, onTap: () -> Unit) =
    KeyCap(label, Modifier.weight(1f).fillMaxWidth(), if (lit) Tone.Enter else Tone.Action, small, repeat = false, onTap)

/** The panels, on the toolbar of every mode. */
@Composable
private fun RowScope.PanelKeys(onKey: (Key) -> Unit, sizes: Boolean) {
    KeyButton("Measures", weight = 2f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Measures)) }
    KeyButton("Currencies", weight = 2f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Currencies)) }
    KeyButton("Sizes", weight = 1.4f, tone = if (sizes) Tone.Enter else Tone.Action, small = true) { onKey(Key.Open(Panel.Sizes)) }
    KeyButton("Trivia", weight = 1.6f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Trivia)) }
    KeyButton("History", weight = 1.6f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.History)) }
}

/** ⌫: a tap removes the character before the cursor; held, it goes on removing, as on every keyboard. */
@Composable
private fun RowScope.BackspaceKey(onKey: (Key) -> Unit, weight: Float = 1f) =
    KeyButton("⌫", weight = weight, tone = Tone.Action, repeat = true) { onKey(Key.Backspace) }

/** A row of keys, sharing the keyboard's height with the other rows. */
@Composable
private fun ColumnScope.KeyRow(content: @Composable RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth().weight(1f), horizontalArrangement = Arrangement.spacedBy(4.dp), content = content)
}

private enum class Tone { Letter, Operator, Action, Enter }

@Composable
private fun RowScope.CharKey(c: Char, write: (String) -> Unit) = KeyButton(c.toString()) { write(c.toString()) }

/** Shift: off, on for the next letter, or locked (two quick taps) until it is tapped again. */
enum class Shift { Off, Once, Locked }

/** Shift's state while the letters are shown, and the letters it makes capitals. Most requests
 *  don't need them (case doesn't matter, but for units such as `GB` and `Gb`). */
class Capitals {
    var shift by mutableStateOf(Shift.Off)
        private set
    private var lastTap = 0L

    /** A tap on ⇧ at `now` (ms): on; tapped again within 0.4 s, locked; else off. */
    fun tap(now: Long) {
        shift = when {
            shift == Shift.Off -> Shift.Once
            shift == Shift.Once && now - lastTap < 400 -> Shift.Locked
            else -> Shift.Off
        }
        lastTap = now
    }

    /** What the key of `c` writes: a capital with Shift on, which then goes off unless locked. */
    fun letter(c: Char): String {
        if (shift == Shift.Off) return c.toString()
        if (shift == Shift.Once) shift = Shift.Off
        return c.uppercase()
    }
}

/** A letter's key: shown and written as a capital while Shift is on. */
@Composable
private fun RowScope.LetterKey(c: Char, capitals: Capitals, write: (String) -> Unit) =
    KeyButton(if (capitals.shift == Shift.Off) c.toString() else c.uppercase()) { write(capitals.letter(c)) }

/** ⇧, its icon telling its state (outlined: off; filled: the next letter; with a bar: locked). */
@Composable
private fun RowScope.ShiftKey(capitals: Capitals) {
    val label = when (capitals.shift) {
        Shift.Off -> "Shift"
        Shift.Once -> "Shift on"
        Shift.Locked -> "Caps lock"
    }
    KeyButton(label, weight = 1.5f, tone = if (capitals.shift == Shift.Off) Tone.Action else Tone.Operator) {
        capitals.tap(android.os.SystemClock.uptimeMillis())
    }
}

/** The keys drawn as an icon: ⌫, ⏎, ⚙ and Shift alone, the panels' keys above their names. The label
 *  stays what the key is called (`SPOKEN`). */
private fun keyIcon(label: String): ImageVector? = when (label) {
    "⌫" -> BackspaceIcon
    "⏎" -> EnterIcon
    "⚙" -> SettingsIcon
    "⌄" -> KeyboardHideIcon
    "⌃" -> KeyboardShowIcon
    "123", "Basic" -> CalculatorIcon
    "ABC" -> KeyboardIcon
    "Sci" -> FunctionsIcon
    "Shift" -> ShiftIcon
    "Shift on" -> ShiftOnIcon
    "Caps lock" -> ShiftLockIcon
    else -> Panel.entries.firstOrNull { it.name == label }?.icon
}

/** The keys that switch the keyboard, drawn as an icon with their name. */
private val KEYBOARDS = setOf("123", "ABC", "Sci", "Basic")

/** What a screen reader says for a key whose label is a symbol (the letters and digits say themselves). */
private val SPOKEN = mapOf(
    "⌫" to "Backspace", "⏎" to "Enter", "+" to "plus", "−" to "minus", "×" to "times", "÷" to "divided by",
    "^" to "to the power of", "(" to "open bracket", ")" to "close bracket", "%" to "percent", "." to "point", ":" to "colon",
    "√" to "square root", "∛" to "cube root", "ˣ√" to "root, with an index", "x!" to "factorial", "x⁻¹" to "inverse, one over",
    "," to "comma", "10ˣ" to "ten to the power of", "EE" to "times ten to the power of", "sinh" to "hyperbolic sine", "cosh" to "hyperbolic cosine", "tanh" to "hyperbolic tangent", "π" to "pi", "°" to "degrees", "123" to "Calculator keys", "ABC" to "Letter keys",
    "Sci" to "Scientific calculator", "Basic" to "Basic calculator", "space" to "Space", "⚙" to "Settings",
    "⌄" to "Hide the keyboard", "⌃" to "Show the keyboard",
)

/**
 * A key: its label, its share of the row's width, what a tap does, and with `repeat` the same again
 * while it is held (after 0.4 s, every 50 ms). A button for accessibility services (TalkBack,
 * Switch Access, Voice Access), named by `SPOKEN`; a ripple while pressed.
 */
@Composable
private fun RowScope.KeyButton(
    label: String,
    weight: Float = 1f,
    tone: Tone = Tone.Letter,
    small: Boolean = false,
    repeat: Boolean = false,
    onTap: () -> Unit,
) = KeyCap(label, Modifier.weight(weight).fillMaxHeight(), tone, small, repeat, onTap)

/** A key, sized by `size` (a share of a row, or of a column). */
@Composable
private fun KeyCap(label: String, size: Modifier, tone: Tone, small: Boolean, repeat: Boolean, onTap: () -> Unit) {
    val haptic = LocalHapticFeedback.current
    val colors = MaterialTheme.colorScheme
    val (background, foreground) = when (tone) {
        Tone.Letter -> colors.surfaceContainerHighest to colors.onSurface
        Tone.Operator -> colors.secondaryContainer to colors.onSecondaryContainer
        Tone.Action -> colors.surfaceContainerHigh to colors.onSurfaceVariant
        Tone.Enter -> colors.primary to colors.onPrimary
    }
    val interactions = remember { MutableInteractionSource() }
    if (repeat) {
        val pressed by interactions.collectIsPressedAsState()
        LaunchedEffect(pressed) {
            if (pressed) {
                delay(400)
                while (true) {
                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    onTap()
                    delay(50)
                }
            }
        }
    }
    val icon = keyIcon(label)
    Surface(
        color = background,
        shape = RoundedCornerShape(8.dp),
        modifier = size
            .clip(RoundedCornerShape(8.dp))
            .combinedClickable(
                interactionSource = interactions,
                indication = ripple(),
                role = Role.Button,
                onClickLabel = SPOKEN[label] ?: label,
                // Held (repeating), the release is no tap.
                onLongClick = if (repeat) ({}) else null,
                onClick = { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); onTap() },
            )
            .semantics { contentDescription = SPOKEN[label] ?: label },
    ) {
        Box(contentAlignment = Alignment.Center) {
            when {
                icon == null -> Text(label, color = foreground, fontSize = if (small) 13.sp else 20.sp, maxLines = 1)
                // A panel's or a keyboard's key: its icon, its name under it.
                Panel.entries.any { it.name == label } || label in KEYBOARDS -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Icon(icon, contentDescription = null, tint = foreground, modifier = Modifier.size(18.dp))
                    Text(label, color = foreground, fontSize = 11.sp, lineHeight = 13.sp, maxLines = 1)
                }
                else -> Icon(icon, contentDescription = null, tint = foreground, modifier = Modifier.size(22.dp))
            }
        }
    }
}
