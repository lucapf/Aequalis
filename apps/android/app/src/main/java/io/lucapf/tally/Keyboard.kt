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
val KeyboardHeight: Dp = 336.dp

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
}

enum class Panel { Measures, Currencies, Trivia }

/**
 * Tally's own keyboard: the letters as on a phone's (lower case only: what is written is matched
 * in any case, and the units whose case matters, `MB` and `Mb`, come from the suggestions and the
 * Measures), the numbers always there, the calculator's keys, and the three panels.
 */
@Composable
fun TallyKeyboard(onKey: (Key) -> Unit, modifier: Modifier = Modifier) {
    val write = { s: String -> onKey(Key.Write(s)) }
    Surface(color = MaterialTheme.colorScheme.surfaceContainer, modifier = modifier) {
        Column(
            Modifier.fillMaxWidth().navigationBarsPadding().height(KeyboardHeight).padding(4.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            KeyRow { "1234567890".forEach { c -> CharKey(c, write) } }
            KeyRow { "qwertyuiop".forEach { c -> CharKey(c, write) } }
            // The middle row is half a key shorter on each side, as on a phone.
            KeyRow {
                Spacer(Modifier.weight(0.5f))
                "asdfghjkl".forEach { c -> CharKey(c, write) }
                Spacer(Modifier.weight(0.5f))
            }
            KeyRow {
                Spacer(Modifier.weight(0.5f))
                "zxcvbnm".forEach { c -> CharKey(c, write) }
                KeyButton("⌫", weight = 2f, tone = Tone.Action, onLongPress = { onKey(Key.Clear) }) { onKey(Key.Backspace) }
                Spacer(Modifier.weight(0.5f))
            }
            // Shown as people write them, written as Tally reads them: × is *, ÷ is /, − is -.
            KeyRow {
                listOf("+" to "+", "−" to "-", "×" to "*", "÷" to "/", "^" to "^", "(" to "(", ")" to ")", "%" to "%", "." to ".", ":" to ":")
                    .forEach { (shown, written) -> KeyButton(shown, tone = Tone.Operator) { write(written) } }
            }
            KeyRow {
                KeyButton("Measures", weight = 2f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Measures)) }
                KeyButton("Currencies", weight = 2f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Currencies)) }
                KeyButton("Trivia", weight = 1.6f, tone = Tone.Action, small = true) { onKey(Key.Open(Panel.Trivia)) }
                KeyButton(",") { write(",") }
                KeyButton("space", weight = 2.4f, small = true) { write(" ") }
                KeyButton("⏎", weight = 1.4f, tone = Tone.Enter) { onKey(Key.Enter) }
            }
        }
    }
}

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
