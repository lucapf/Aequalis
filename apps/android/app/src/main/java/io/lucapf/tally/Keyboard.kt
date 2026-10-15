package io.lucapf.tally

import androidx.compose.foundation.gestures.detectTapGestures
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
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The keyboard's height: the Measures, Currencies and Trivia panels take its place exactly. */
val KeyboardHeight: Dp = 380.dp

/** What a key does. */
sealed interface Key {
    /** Writes this text. */
    data class Write(val text: String) : Key
    data object Backspace : Key
    /** A long press on ⌫: the request cleared. */
    data object Clear : Key
    data object Enter : Key
    /** Opens a panel over the keyboard. */
    data class Open(val panel: Panel) : Key
    /** Switches the keyboard: letters, or the calculator (basic or scientific). */
    data class Switch(val mode: Mode) : Key
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
                when (mode) {
                    Mode.Text -> KeyButton("123", weight = 1.2f, tone = Tone.Action, small = true) { onKey(Key.Switch(Mode.Basic)) }
                    else -> KeyButton("ABC", weight = 1.2f, tone = Tone.Action, small = true) { onKey(Key.Switch(Mode.Text)) }
                }
                when (mode) {
                    Mode.Text -> {}
                    Mode.Basic -> KeyButton("Sci", weight = 1.2f, tone = Tone.Action, small = true) { onKey(Key.Switch(Mode.Scientific)) }
                    Mode.Scientific -> KeyButton("Basic", weight = 1.2f, tone = Tone.Action, small = true) { onKey(Key.Switch(Mode.Basic)) }
                }
                PanelKeys(onKey)
            }
            when (mode) {
                Mode.Text -> Letters(onKey)
                Mode.Basic, Mode.Scientific -> Calculator(scientific = mode == Mode.Scientific, onKey)
            }
        }
    }
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
            keys.forEach { k ->
                when {
                    k.writes != null -> KeyButton(k.label, tone = k.tone) { write(k.writes) }
                    k.label == "⌫" -> BackspaceKey(onKey)
                    else -> KeyButton(k.label, tone = k.tone) { onKey(Key.Enter) }
                }
            }
        }
    }
}

/** The three panels, on the toolbar of every mode. */
@Composable
private fun RowScope.PanelKeys(onKey: (Key) -> Unit) {
    KeyButton("Measures", weight = 2f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Measures)) }
    KeyButton("Currencies", weight = 2f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Currencies)) }
    KeyButton("Trivia", weight = 1.6f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Trivia)) }
}

/** ⌫: a tap removes the character before the cursor, a long press the whole request. */
@Composable
private fun RowScope.BackspaceKey(onKey: (Key) -> Unit, weight: Float = 1f) =
    KeyButton("⌫", weight = weight, tone = Tone.Action, onLongPress = { onKey(Key.Clear) }) { onKey(Key.Backspace) }

/** A row of keys, sharing the keyboard's height with the other rows. */
@Composable
private fun ColumnScope.KeyRow(content: @Composable RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth().weight(1f), horizontalArrangement = Arrangement.spacedBy(4.dp), content = content)
}

private enum class Tone { Letter, Operator, Action, Enter }

@Composable
private fun RowScope.CharKey(c: Char, write: (String) -> Unit) = KeyButton(c.toString()) { write(c.toString()) }

/** A key: its label, its share of the row's width, and what a tap (and a long press) does. */
@Composable
private fun RowScope.KeyButton(
    label: String,
    weight: Float = 1f,
    tone: Tone = Tone.Letter,
    small: Boolean = false,
    onLongPress: (() -> Unit)? = null,
    onTap: () -> Unit,
) {
    val haptic = LocalHapticFeedback.current
    val colors = MaterialTheme.colorScheme
    val (background, foreground) = when (tone) {
        Tone.Letter -> colors.surfaceContainerHighest to colors.onSurface
        Tone.Operator -> colors.secondaryContainer to colors.onSecondaryContainer
        Tone.Action -> colors.surfaceContainerHigh to colors.onSurfaceVariant
        Tone.Enter -> colors.primary to colors.onPrimary
    }
    Surface(
        color = background,
        shape = RoundedCornerShape(8.dp),
        modifier = Modifier.weight(weight).fillMaxHeight().pointerInput(onTap, onLongPress) {
            detectTapGestures(
                onTap = { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); onTap() },
                onLongPress = onLongPress?.let { long -> { haptic.performHapticFeedback(HapticFeedbackType.LongPress); long() } },
            )
        },
    ) {
        Box(contentAlignment = Alignment.Center) {
            Text(label, color = foreground, fontSize = if (small) 13.sp else 20.sp, maxLines = 1)
        }
    }
}
