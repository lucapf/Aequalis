package io.lucapf.tally

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/**
 * A panel over the keyboard (the same height): Measures (the unit classes, then a class's units),
 * Currencies, or Trivia (the sections, then their values). `write` gets what a tap writes into the
 * request; `close` goes back to the keyboard.
 */
@Composable
fun PanelView(panel: Panel, write: (String) -> Unit, close: () -> Unit) {
    // The second level (a class, a section), when one is open, and in the Trivia a third, a value's
    // details: the back gesture goes up one level.
    var opened by remember(panel) { mutableStateOf<String?>(null) }
    var detail by remember(panel) { mutableStateOf<String?>(null) }
    val up = {
        when {
            detail != null -> detail = null
            opened != null -> opened = null
            else -> close()
        }
    }
    BackHandler(onBack = up)
    Surface(color = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().height(KeyboardHeight)) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                val back = when {
                    detail != null -> "Back to $opened"
                    opened != null -> "Back to ${panel.name}"
                    else -> "Back to the keyboard"
                }
                TextButton(onClick = up, modifier = Modifier.semantics { contentDescription = back }) {
                    Text(if (opened != null) "←" else "⌨", modifier = Modifier.clearAndSetSemantics {})
                }
                Text(detail ?: opened ?: panel.name, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                TextButton(onClick = close, modifier = Modifier.semantics { contentDescription = "Close" }) {
                    Text("✕", modifier = Modifier.clearAndSetSemantics {})
                }
            }
            HorizontalDivider()
            when (panel) {
                Panel.Measures -> MeasuresPanel(opened, open = { opened = it }, write)
                Panel.Currencies -> CurrenciesPanel(write)
                Panel.Trivia -> TriviaPanel(opened, detail, open = { opened = it }, show = { detail = it }, write)
            }
        }
    }
}

/** The unit classes; a class opened: its units, by its two systems (Metric and Imperial…). */
@Composable
private fun MeasuresPanel(opened: String?, open: (String) -> Unit, write: (String) -> Unit) {
    val measures = remember { Core.measures() }
    val measure = measures.firstOrNull { it.name == opened }
    if (measure == null) {
        LazyColumn {
            items(measures) { m -> Line(m.name, m.units.take(5).joinToString(" · ") { it.symbol }) { open(m.name) } }
        }
        return
    }
    // A system tab, when the class has units on both (Date & Time has durations only).
    val tabs = listOf("metric" to measure.systems[0], "imperial" to measure.systems[1])
        .filter { (system, _) -> measure.units.any { it.system == system } }
    var tab by remember(measure.name) { mutableStateOf(tabs.first().first) }
    Column {
        if (tabs.size > 1) {
            Row(Modifier.padding(horizontal = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                tabs.forEach { (system, title) -> FilterChip(selected = tab == system, onClick = { tab = system }, label = { Text(title) }) }
            }
        }
        // A unit of both systems (`b`, `B`) on each tab.
        val units = measure.units.filter { it.system == tab || it.system == "both" }
        LazyVerticalGrid(GridCells.Fixed(3), Modifier.padding(8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            items(units) { u ->
                OutlinedButton(onClick = { write(u.symbol) }) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Text(u.symbol, style = MaterialTheme.typography.titleMedium, maxLines = 1)
                        // `light-year (Julian year of light travel)`: the name, not the note.
                        Text(u.name.substringBefore(" ("), style = MaterialTheme.typography.labelSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}

/** Every currency, the most used first: a tap writes its code. */
@Composable
private fun CurrenciesPanel(write: (String) -> Unit) {
    val currencies = remember { Core.currencies() }
    LazyColumn {
        items(currencies) { c -> Line("${c.code}  ${c.name}", c.symbols.joinToString(" ")) { write(c.code) } }
    }
}

/** The Trivia's sections; a section opened: its values now; a value tapped: its details, as the
 *  TUI's popup (the description, then each line), and a button that writes it into the request. */
@Composable
private fun TriviaPanel(opened: String?, detail: String?, open: (String) -> Unit, show: (String) -> Unit, write: (String) -> Unit) {
    val sections = remember { Core.trivia() }
    val section = sections.firstOrNull { it.name == opened }
    val entry = section?.entries?.firstOrNull { it.name == detail }
    when {
        section == null -> LazyColumn {
            items(sections) { s -> Line(s.name, s.entries.take(3).joinToString(" · ") { it.name }) { open(s.name) } }
        }
        entry == null -> LazyColumn {
            items(section.entries) { e -> Line(e.name, e.value.ifEmpty { e.short }) { show(e.name) } }
        }
        else -> TriviaDetail(entry, write)
    }
}

/** A Trivia value in full, as the TUI's popup: the description, then every line (the value among
 *  them); links open in the browser. The request gets the value only from the button. */
@Composable
private fun TriviaDetail(e: TriviaEntry, write: (String) -> Unit) {
    val links = LocalUriHandler.current
    Column(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.weight(1f).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (e.description.isNotEmpty()) item { Text(e.description, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(top = 8.dp)) }
            items(e.details) { (label, text) ->
                val link = text.startsWith("https://")
                Column(if (link) Modifier.clickable(role = Role.Button, onClickLabel = "Open") { links.openUri(text) } else Modifier) {
                    Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(text, style = MaterialTheme.typography.bodyMedium, color = if (link) MaterialTheme.colorScheme.primary else Color.Unspecified)
                }
            }
        }
        if (e.written.isNotBlank()) {
            Button(onClick = { write(e.written.trim()) }, modifier = Modifier.fillMaxWidth().padding(8.dp)) {
                Text("Use in the request: ${e.written.trim()}", maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** A row of a panel's list: a title, a line below it, a tap. */
@Composable
private fun Line(title: String, detail: String, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 10.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge)
        if (detail.isNotEmpty()) {
            Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}
