package io.lucapf.tally

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/** Every setting, changed in place and saved at once (the core checks each value). */
@Composable
fun SettingsScreen(close: () -> Unit, modifier: Modifier = Modifier) {
    BackHandler(onBack = close)
    // Read again after each change: the core normalises the value (`ON` is `true`).
    var settings by remember { mutableStateOf(Core.settings()) }
    var message by remember { mutableStateOf<String?>(null) }
    val change = { key: String, value: String ->
        message = Core.change(key, value)
        settings = Core.settings()
    }
    Column(modifier.fillMaxSize().imePadding()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = close) { Text("←") }
            Text("Settings", style = MaterialTheme.typography.titleLarge)
        }
        message?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(horizontal = 16.dp)) }
        LazyColumn {
            items(settings, key = { it.key }) { s ->
                SettingRow(s, change)
                HorizontalDivider()
            }
        }
    }
}

@Composable
private fun SettingRow(s: Setting, change: (String, String) -> Unit) {
    Column(Modifier.fillMaxWidth().padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(s.key, style = MaterialTheme.typography.titleSmall)
                Text(s.about, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (s.kind == "bool") {
                Switch(checked = s.value == "true", onCheckedChange = { change(s.key, it.toString()) })
            }
        }
        when (s.kind) {
            "choice" -> FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                s.options.forEach { o -> FilterChip(selected = s.value == o, onClick = { change(s.key, o) }, label = { Text(o) }) }
            }
            // Free text (an address, a date pattern, a currency): the phone's keyboard, then Save.
            "text" -> {
                var draft by remember(s.value) { mutableStateOf(s.value) }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(draft, onValueChange = { draft = it }, singleLine = true, modifier = Modifier.weight(1f))
                    TextButton(onClick = { change(s.key, draft) }, enabled = draft != s.value) { Text("Save") }
                    TextButton(onClick = { change(s.key, "") }, enabled = s.value != s.default) { Text("Default") }
                }
            }
        }
    }
}
