package io.lucapf.tally

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
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

/** The request, and below it the result, the error, or the choices of an ambiguous request. */
@Composable
fun TallyScreen(modifier: Modifier = Modifier) {
    // The state: what is typed, and whether Enter was pressed on it. Compose draws the screen from
    // them, and redraws it whenever they change.
    var input by remember { mutableStateOf("") }
    var submitted by remember { mutableStateOf(false) }
    // Evaluated again only when the input changes (the core answers in well under a millisecond).
    val reply = remember(input) { if (input.isBlank()) null else Core.evaluate(input) }
    // While a word is being typed (no space after it, no Enter), only a result shows: `Rie` on the way
    // to `Rieti to Rome` is no mistake. The same rule as the TUI and the Omarchy widget.
    val finished = submitted || input.endsWith(" ")

    Column(modifier.padding(16.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedTextField(
            value = input,
            onValueChange = { input = it; submitted = false },
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
            reply.choices.isNotEmpty() -> {
                Text(reply.question ?: "Which one?", style = MaterialTheme.typography.titleMedium)
                // Each choice is a complete request: choosing it writes it in.
                reply.choices.forEach { choice ->
                    OutlinedButton(onClick = { input = choice.request; submitted = true }, modifier = Modifier.fillMaxWidth()) {
                        Text(choice.label)
                    }
                }
            }
            else -> Text(reply.error ?: "", color = MaterialTheme.colorScheme.error)
        }
    }
}
