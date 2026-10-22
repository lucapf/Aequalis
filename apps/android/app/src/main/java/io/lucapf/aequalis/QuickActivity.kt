package io.lucapf.aequalis

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.ViewGroup
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import io.lucapf.aequalis.ui.theme.Appearance
import io.lucapf.aequalis.ui.theme.Typography
import io.lucapf.aequalis.ui.theme.backdrop
import io.lucapf.aequalis.ui.theme.colorScheme
import io.lucapf.aequalis.ui.theme.isDark
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Aequalis from anywhere, in a small window over the app in use: text shared to it (`ACTION_SEND`),
 * the Quick Settings tile (`QuickTile`) and the icon's shortcut (res/xml/shortcuts.xml). Not in the
 * text-selection menu: too much in every app. The phone's keyboard, the answer as you type, Copy.
 */
class QuickActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Core.startInBackground(applicationContext)
        Appearance.load(applicationContext)
        History.load(applicationContext)
        // A dialog's window is narrow: as wide as the screen allows (its margins kept).
        window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
        val text = given(intent)
        setContent {
            QuickTheme {
                QuickInput(
                    initial = text,
                    open = { request ->
                        startActivity(
                            Intent(this, MainActivity::class.java).putExtra(MainActivity.REQUEST, request)
                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK),
                        )
                        finish()
                    },
                    close = { finish() },
                )
            }
        }
    }

    companion object {
        /** The text the window opens with: what was shared, or nothing. */
        fun given(intent: Intent?): String = oneLine(if (intent?.action == Intent.ACTION_SEND) intent.getStringExtra(Intent.EXTRA_TEXT).orEmpty() else "")

        /** A request is one line, at most the core's length. */
        private fun oneLine(text: String): String = text.replace(Regex("\\s+"), " ").trim().take(Core.maxInput)
    }
}

/** The app's colours (the skin and the mode chosen), on a card rather than the whole screen. */
@Composable
private fun QuickTheme(content: @Composable () -> Unit) {
    val dark = isDark()
    val scheme = colorScheme(LocalContext.current, Appearance.skin, dark)
    MaterialTheme(colorScheme = scheme, typography = Typography) {
        // The text in the theme's colour: no full-screen Surface here to give it.
        Surface(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(24.dp)).background(backdrop(Appearance.skin, dark, scheme)),
            color = scheme.surfaceContainer,
            contentColor = scheme.onSurface,
        ) { Column { content() } }
    }
}

/** The request (the phone's keyboard), the answer as it is typed, and what to do with it. */
@Composable
private fun QuickInput(initial: String, open: (String) -> Unit, close: () -> Unit) {
    val context = LocalContext.current
    var field by rememberSaveable(stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue(initial, TextRange(initial.length))) }
    var submitted by rememberSaveable { mutableStateOf(false) }
    val request = field.text
    val reply by produceState<Reply?>(null, request, submitted) {
        value = if (request.isBlank()) null else withContext(Dispatchers.Default) { Core.evaluate(request, submitted) }
    }
    // The keyboard at once, but only once the window has the focus: focused earlier (the window
    // not shown yet, the screen off or locked), Compose fails bringing the field into view.
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    val shown = LocalWindowInfo.current.isWindowFocused
    LaunchedEffect(shown) {
        if (shown && !focused) {
            focused = true
            focus.requestFocus()
        }
    }
    val result = reply?.takeIf { it.show == Show.Result }
    // Used: kept in the history, as in the app.
    val keep = { r: Reply -> History.add(request, r.result.orEmpty(), r.value ?: r.result.orEmpty()) }
    val copy = { text: String ->
        context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Aequalis", text))
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) Toast.makeText(context, "Copied", Toast.LENGTH_SHORT).show()
    }
    Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            AequalisMark(q = MaterialTheme.colorScheme.primary, modifier = Modifier.size(width = 26.dp, height = 24.dp))
            Text("  Aequalis", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
            TextButton(onClick = close) { Text("Close") }
        }
        OutlinedTextField(
            field,
            onValueChange = { field = it; submitted = false },
            singleLine = true,
            placeholder = { Text("10 km mi, 12 usd eur, now Tokyo…") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Text, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { submitted = true }),
            modifier = Modifier.fillMaxWidth().focusRequester(focus),
        )
        // The answer: the result large; a hint, a question or an error as the core words it.
        when {
            result != null -> {
                val text = result.result.orEmpty()
                if ('\n' in text && result.extra.isNotEmpty()) PeopleSizes(result.extra)
                else Text(text, style = resultStyle(text, landscape = false))
                Extras(result)
                result.details.forEach { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            }
            reply?.show == Show.Question -> {
                Text(reply!!.message, style = MaterialTheme.typography.bodyMedium)
                reply!!.choices.forEach { c ->
                    OutlinedButton(onClick = { field = TextFieldValue(c.request, TextRange(c.request.length)); submitted = true }, modifier = Modifier.fillMaxWidth()) { Text(c.label) }
                }
            }
            reply?.show == Show.Hint -> {
                Text(reply!!.message, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Extras(reply!!)
            }
            reply?.show == Show.Error -> Text(reply!!.message, color = MaterialTheme.colorScheme.error)
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (result != null) {
                val value = result.value ?: result.result.orEmpty()
                val whole = result.expression ?: result.result.orEmpty()
                OutlinedButton(onClick = { keep(result); copy(value); close() }) { Text("Copy $value") }
                OutlinedButton(onClick = { keep(result); copy(whole); close() }) { Text("Copy expression") }
            }
            TextButton(onClick = { open(request) }) { Text("Open in Aequalis") }
        }
    }
}
