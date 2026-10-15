package io.lucapf.tally

import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ripple
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay

/** The keyboard's height: the Measures, Currencies and Trivia panels take its place exactly. */
val KeyboardHeight: Dp = 380.dp

/** What a key does. */
sealed interface Key {
    /** Writes this text. */
    data class Write(val text: String) : Key
    data object Backspace : Key
    /** The request cleared (the C key). */
    data object Clear : Key
    data object Enter : Key
    /** Opens a panel over the keyboard. */
    data class Open(val panel: Panel) : Key
    /** Switches the keyboard: letters, or the calculator (basic or scientific). */
    data class Switch(val mode: Mode) : Key
    /** Opens the Settings (the landscape toolbar; in portrait, ⚙ at the top). */
    data object Settings : Key
}

enum class Panel { Measures, Currencies, Trivia }

/** The keyboard's layouts: letters (with the digits and operators), or the calculator's keys. */
enum class Mode { Text, Basic, Scientific }

/**
 * Tally's own keyboard, in one of its `mode`s. Letters: as on a phone (lower case only: what is
 * written is matched in any case, and the units whose case matters, `MB` and `Mb`, come from the
 * suggestions and the Measures), with the digits and the operators above them. The calculator: the
 * digits and operators in a grid, and in scientific mode the functions and constants beside them.
 */
@Composable
fun TallyKeyboard(mode: Mode, onKey: (Key) -> Unit, modifier: Modifier = Modifier) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainer, modifier = modifier) {
        Column(
            Modifier.fillMaxWidth().navigationBarsPadding().height(KeyboardHeight).padding(4.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            // The toolbar, in every mode: the keyboards and the panels, apart from the keys typed.
            KeyRow {
                // Clearing has its own key: no gesture on ⌫ wipes the request by surprise.
                KeyButton("C", weight = 0.8f, tone = Tone.Action) { onKey(Key.Clear) }
                switches(mode).forEach { (label, to) -> KeyButton(label, weight = 1.2f, tone = Tone.Action, small = true) { onKey(Key.Switch(to)) } }
                PanelKeys(onKey)
            }
            when (mode) {
                Mode.Text -> Letters(onKey)
                Mode.Basic, Mode.Scientific -> Calculator(scientific = mode == Mode.Scientific, onKey)
            }
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

/** The operators as people write them, written as Tally reads them: × is *, ÷ is /, − is -. */
private val OPERATORS = listOf("+" to "+", "−" to "-", "×" to "*", "÷" to "/", "^" to "^", "(" to "(", ")" to ")", "%" to "%", "." to ".", ":" to ":")

@Composable
private fun ColumnScope.Letters(onKey: (Key) -> Unit) {
    val write = { s: String -> onKey(Key.Write(s)) }
    // Above the digits, next to them: the request's numbers and operators are typed together.
    KeyRow { OPERATORS.forEach { (shown, written) -> KeyButton(shown, tone = Tone.Operator) { write(written) } } }
    KeyRow { "1234567890".forEach { c -> CharKey(c, write) } }
    KeyRow { "qwertyuiop".forEach { c -> CharKey(c, write) } }
    // The middle rows are half a key shorter on each side, as on a phone.
    KeyRow {
        Spacer(Modifier.weight(0.5f))
        "asdfghjkl".forEach { c -> CharKey(c, write) }
        Spacer(Modifier.weight(0.5f))
    }
    KeyRow {
        Spacer(Modifier.weight(0.5f))
        "zxcvbnm".forEach { c -> CharKey(c, write) }
        BackspaceKey(onKey, weight = 2f)
        Spacer(Modifier.weight(0.5f))
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

/** Scientific: two columns beside the grid. Each function writes its bracket (`sin(`), `)` closes
 *  it; ° makes the angle degrees (`sin(30°)`). */
private val SCIENCE = listOf(
    listOf("sin" to "sin(", "cos" to "cos("),
    listOf("tan" to "tan(", "√" to "sqrt("),
    listOf("ln" to "ln(", "log" to "log("),
    listOf("π" to "pi", "e" to "e"),
    listOf("^" to "^", "°" to "°"),
)

/** The calculator: digits and operators in a grid; scientific: two columns of functions on the left. */
@Composable
private fun ColumnScope.Calculator(scientific: Boolean, onKey: (Key) -> Unit) {
    val write = { s: String -> onKey(Key.Write(s)) }
    CALCULATOR.forEachIndexed { i, keys ->
        KeyRow {
            if (scientific) {
                SCIENCE[i].forEach { (shown, written) -> KeyButton(shown, tone = Tone.Action, small = true) { write(written) } }
                Spacer(Modifier.width(6.dp))
            }
            keys.forEach { k -> CalcKey(k, onKey) }
        }
    }
}

/** A key's height in landscape: about the portrait one, so the request and the result keep the
 *  rest of the screen. */
private val LandscapeRow: Dp = 44.dp

/** Landscape, on the left: the toolbar, as a column (C, the keyboards, the panels, Settings). */
@Composable
fun SideToolbar(mode: Mode, onKey: (Key) -> Unit, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxHeight().width(100.dp).padding(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        ColumnKey("C") { onKey(Key.Clear) }
        switches(mode).forEach { (label, to) -> ColumnKey(label, small = true) { onKey(Key.Switch(to)) } }
        Panel.entries.forEach { p -> ColumnKey(p.name, small = true) { onKey(Key.Open(p)) } }
        ColumnKey("⚙") { onKey(Key.Settings) }
    }
}

/** Landscape, under the request: the letters, as in portrait, with ⌫, space and ⏎. */
@Composable
fun LetterPad(onKey: (Key) -> Unit, modifier: Modifier = Modifier) {
    val write = { s: String -> onKey(Key.Write(s)) }
    Column(modifier.fillMaxWidth().padding(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        PadRow { "qwertyuiop".forEach { c -> CharKey(c, write) } }
        PadRow {
            Spacer(Modifier.weight(0.5f))
            "asdfghjkl".forEach { c -> CharKey(c, write) }
            Spacer(Modifier.weight(0.5f))
        }
        PadRow {
            Spacer(Modifier.weight(0.5f))
            "zxcvbnm".forEach { c -> CharKey(c, write) }
            BackspaceKey(onKey, weight = 2f)
            Spacer(Modifier.weight(0.5f))
        }
        PadRow {
            KeyButton("space", weight = 4f, small = true) { write(" ") }
            KeyButton("⏎", weight = 1.4f, tone = Tone.Enter) { onKey(Key.Enter) }
        }
    }
}

/** Landscape, on the right: the digits and operators in the calculator's grid, the functions beside
 *  them in scientific mode. With the letters (whose rows have ⌫ and ⏎), its last row is `0 . : ^`. */
@Composable
fun NumberPad(mode: Mode, onKey: (Key) -> Unit, modifier: Modifier = Modifier) {
    val write = { s: String -> onKey(Key.Write(s)) }
    val rows = if (mode == Mode.Text) CALCULATOR.dropLast(1) + listOf(listOf(digit("0"), digit("."), op(":", ":"), op("^", "^"))) else CALCULATOR
    val columns = if (mode == Mode.Scientific) 6 else 4
    Column(modifier.width(58.dp * columns).padding(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        rows.forEachIndexed { i, keys ->
            PadRow {
                if (mode == Mode.Scientific) {
                    SCIENCE[i].forEach { (shown, written) -> KeyButton(shown, tone = Tone.Action, small = true) { write(written) } }
                }
                keys.forEach { k -> CalcKey(k, onKey) }
            }
        }
    }
}

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
private fun ColumnScope.ColumnKey(label: String, small: Boolean = false, onTap: () -> Unit) =
    KeyCap(label, Modifier.weight(1f).fillMaxWidth(), Tone.Action, small, repeat = false, onTap)

/** The three panels, on the toolbar of every mode. */
@Composable
private fun RowScope.PanelKeys(onKey: (Key) -> Unit) {
    KeyButton("Measures", weight = 2f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Measures)) }
    KeyButton("Currencies", weight = 2f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Currencies)) }
    KeyButton("Trivia", weight = 1.6f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Trivia)) }
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

/** What a screen reader says for a key whose label is a symbol (the letters and digits say themselves). */
private val SPOKEN = mapOf(
    "⌫" to "Backspace", "⏎" to "Enter", "C" to "Clear", "+" to "plus", "−" to "minus", "×" to "times", "÷" to "divided by",
    "^" to "to the power of", "(" to "open bracket", ")" to "close bracket", "%" to "percent", "." to "point", ":" to "colon",
    "√" to "square root", "π" to "pi", "°" to "degrees", "123" to "Calculator keys", "ABC" to "Letter keys",
    "Sci" to "Scientific calculator", "Basic" to "Basic calculator", "space" to "Space", "⚙" to "Settings",
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
            Text(label, color = foreground, fontSize = if (small) 13.sp else 20.sp, maxLines = 1)
        }
    }
}
