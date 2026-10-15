package io.lucapf.tally

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
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
                Scaffold(modifier = Modifier.fillMaxSize()) { innerPadding ->
                    TallyScreen(Modifier.padding(innerPadding))
                }
            }
        }
    }
}

/** The request, and below it the result, the error, or the choices of an ambiguous request; above
 *  the keyboard, the words offered for the word being typed. */
@Composable
fun TallyScreen(modifier: Modifier = Modifier) {
    // The state: what is typed with the cursor (a `TextFieldValue`: text and selection), and whether
    // Enter was pressed on it. Compose draws the screen from them, and redraws it when they change.
    var field by remember { mutableStateOf(TextFieldValue("")) }
    var submitted by remember { mutableStateOf(false) }
    val input = field.text
    // Evaluated again only when the input changes (the core answers in well under a millisecond).
    val reply = remember(input) { if (input.isBlank()) null else Core.evaluate(input) }
    // While a word is being typed (no space after it, no Enter), only a result shows: `Rie` on the way
    // to `Rieti to Rome` is no mistake. The same rule as the TUI and the Omarchy widget.
    val finished = submitted || input.endsWith(" ")
    // The words offered for the word at the cursor (none while text is selected).
    val cursor = field.selection.end
    val before = input.take(cursor)
    val offered = remember(before, field.selection.collapsed) {
        if (field.selection.collapsed) Core.suggestions(before) else Suggestions(0, emptyList())
    }

    // `imePadding`: the column ends where the keyboard begins, so the words sit right above it.
    Column(modifier.fillMaxSize().imePadding().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedTextField(
            value = field,
            onValueChange = { field = it; submitted = false },
            label = { Text("10 km mi, 12+4/2, now Tokyo…") },
            singleLine = true,
            // No autocorrect or capitals: the keyboard would "fix" `km mi` and `usd`.
            keyboardOptions = KeyboardOptions(
                autoCorrectEnabled = false,
                capitalization = KeyboardCapitalization.None,
                imeAction = ImeAction.Done,
            ),
            keyboardActions = KeyboardActions(onDone = { submitted = true }),
            modifier = Modifier.fillMaxWidth(),
        )
        when {
            reply == null -> {}
            reply.result != null -> Text(reply.result, style = MaterialTheme.typography.headlineMedium)
            !finished -> {}
            // What comes next (`now the length unit to convert to: in, ft, yd…`): a hint, in grey.
            reply.hint != null -> Text(reply.hint, color = MaterialTheme.colorScheme.onSurfaceVariant)
            reply.choices.isNotEmpty() -> {
                Text(reply.question ?: "Which one?", style = MaterialTheme.typography.titleMedium)
                // Each choice is a complete request: choosing it writes it in.
                reply.choices.forEach { choice ->
                    OutlinedButton(onClick = { field = TextFieldValue(choice.request, TextRange(choice.request.length)); submitted = true }, modifier = Modifier.fillMaxWidth()) {
                        Text(choice.label)
                    }
                }
            }
            else -> Text(reply.error ?: "", color = MaterialTheme.colorScheme.error)
        }
        Spacer(Modifier.weight(1f))
        // One tap writes the word in place of what is typed of it, then a space: the next word follows.
        LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(offered.items) { s ->
                SuggestionChip(
                    onClick = {
                        val after = input.drop(cursor)
                        val written = input.take(offered.start) + s.write + " "
                        field = TextFieldValue(written + after.trimStart(), TextRange(written.length))
                        submitted = false
                    },
                    label = { Text(s.label) },
                )
            }
        }
    }
}
