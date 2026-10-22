package io.lucapf.aequalis

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.runtime.withFrameNanos
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.material3.VerticalDivider
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * A panel over the keyboard (the same height): Measures (the unit classes, then a class's units),
 * Currencies, Trivia (the sections, then their values), or History. `write` gets what a tap writes
 * into the request; `close` goes back to the keyboard.
 */
@Composable
fun PanelView(
    panel: Panel,
    write: (String) -> Unit,
    close: () -> Unit,
    openLink: (String) -> Unit,
    // The class of the request's unit, from the core: Measures opens at it.
    unitClass: String?,
    // The History's: a request back in the line, a result copied, an entry shared.
    recall: (String) -> Unit,
    copy: (String) -> Unit,
    share: (String) -> Unit,
    // In portrait the keyboard's place; in landscape, the space under the request.
    size: Modifier = Modifier.fillMaxWidth().height(KeyboardHeight),
    // Landscape: each level a column beside the previous one (the classes, then a class's units;
    // the sections, their values, a value's details), the item chosen highlighted.
    wide: Boolean = false,
    // A large screen: the panel always open beside the keys, the panels as tabs (`select`), no
    // close; a class or a section opened has its title bar, to go back.
    docked: Boolean = false,
    select: (Panel) -> Unit = {},
    // A key of the Sizes (`shoes`, `eu`): the core writes it, `shoes` brought along when missing.
    sizeKey: (String) -> Unit = write,
) {
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
    // Docked, at its first level: nothing to close, the back gesture is the app's.
    BackHandler(enabled = !docked || opened != null, onBack = up)
    Surface(color = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.navigationBarsPadding().then(size)) {
            if (docked) {
                PrimaryTabRow(selectedTabIndex = panel.ordinal, containerColor = Color.Transparent) {
                    Panel.entries.forEach { p ->
                        Tab(selected = p == panel, onClick = { select(p) }, text = { Text(p.name, maxLines = 1) }, icon = { Icon(p.icon, contentDescription = null) })
                    }
                }
            }
            // Wide (landscape), no title bar: the toolbar's key of the panel is lit and closes it, the
            // back gesture goes up a level.
            if (!wide && (!docked || opened != null)) Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                val back = when {
                    detail != null -> "Back to $opened"
                    opened != null -> "Back to ${panel.name}"
                    else -> "Back to the keyboard"
                }
                IconButton(onClick = up) { Icon(if (opened != null) BackIcon else KeyboardIcon, contentDescription = back) }
                // The panel's icon, or the class's or the section's, beside the title (a value's
                // details: none).
                val level = opened
                val icon = when {
                    detail != null -> null
                    level != null -> measureIcon(level) ?: triviaIcon(level)
                    else -> panel.icon
                }
                icon?.let { Icon(it, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(end = 8.dp).size(20.dp)) }
                Text(detail ?: opened ?: panel.name, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                if (!docked) IconButton(onClick = close) { Icon(CloseIcon, contentDescription = "Close") }
            }
            if (!wide && !docked) HorizontalDivider()
            // Another class or section: its own values, no detail of the previous one.
            val open = { name: String -> opened = name; detail = null }
            when (panel) {
                Panel.Measures -> MeasuresPanel(opened, open, write, wide, unitClass, sizeKey)
                Panel.Currencies -> CurrenciesPanel(write, wide)
                Panel.Sizes -> SizesPanel(sizeKey)
                Panel.Trivia -> TriviaPanel(opened, detail, open, show = { detail = it }, write, openLink, wide)
                Panel.History -> HistoryPanel(recall, copy, share)
            }
        }
    }
}

/** A panel's list, loaded off the main thread once the panel opens (the Trivia computes the planets'
 *  distances now); null until it arrives, a frame or two later. */
@Composable
private fun <T> loaded(load: () -> T): T? =
    produceState<T?>(null) { value = withContext(Dispatchers.Default) { load() } }.value

/** The width of a list beside the next level, in landscape. */
private val ListWidth = 230.dp

/** The narrowest a Trivia value's details get beside the lists (landscape): room for its lines. */
private val DetailWidth = 360.dp

/** The unit classes; a class opened: its units, by its two systems (Metric and Imperial…). Wide:
 *  the classes on the left, the units of the one chosen beside them. With a unit in the request
 *  (`10 cm `), its class (`unitClass`, the core's) opened at once: the target is of the same kind. */
@Composable
private fun MeasuresPanel(opened: String?, open: (String) -> Unit, write: (String) -> Unit, wide: Boolean, unitClass: String?, sizeKey: (String) -> Unit) {
    val measures = loaded { Core.measures() } ?: return
    LaunchedEffect(measures) { if (opened == null) unitClass?.let(open) }
    val measure = measures.firstOrNull { it.name == opened }
    val classes = @Composable { modifier: Modifier ->
        LazyColumn(modifier) {
            items(measures) { m -> Line(m.name, m.units.take(5).joinToString(" · ") { it.symbol }, icon = measureIcon(m.name), selected = m.name == opened) { open(m.name) } }
            // The sizes, a class of their own: their groups and system keys.
            item { Line(SIZES, "shoes · EU UK US JP CN", icon = SizesIcon, selected = opened == SIZES) { open(SIZES) } }
        }
    }
    when {
        wide -> Row(Modifier.fillMaxSize()) {
            classes(Modifier.width(ListWidth).fillMaxHeight())
            VerticalDivider()
            if (opened == SIZES) SizesPanel(sizeKey) else measure?.let { MeasureUnits(it, write, columns = GridCells.Adaptive(96.dp)) }
        }
        opened == SIZES -> SizesPanel(sizeKey)
        measure == null -> classes(Modifier)
        else -> MeasureUnits(measure, write, columns = GridCells.Fixed(3))
    }
}

/** The Sizes as a strip over the keyboard (the Sizes key shows and hides it): the groups (Shoes) and
 *  the systems as chips, in a row that scrolls; a tap writes it (`sizeKey`). The keys stay. */
@Composable
fun SizesStrip(sizeKey: (String) -> Unit) {
    val sizes = loaded { Core.sizes() } ?: return
    LazyRow(Modifier.fillMaxWidth().padding(horizontal = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        items(sizes.groups) { (name, word) ->
            AssistChip(onClick = { sizeKey(word) }, label = { Text(name) }, leadingIcon = { Icon(ShoesIcon, contentDescription = null, modifier = Modifier.size(18.dp)) })
        }
        items(sizes.systems) { (name, code) -> AssistChip(onClick = { sizeKey(code) }, label = { Text(name) }) }
    }
}

/** The Sizes class's name, in the Measures. */
private const val SIZES = "Sizes"

/** The Sizes: the groups (Shoes), which write their word, then the system keys, which write their
 *  code (`eu`), as buttons. */
@Composable
private fun SizesPanel(write: (String) -> Unit) {
    val sizes = loaded { Core.sizes() } ?: return
    Column {
        sizes.groups.forEach { (name, word) -> Line(name, "men, women, kids: 42 eu men shoes us", icon = ShoesIcon) { write(word) } }
        HorizontalDivider()
        LazyVerticalGrid(GridCells.Adaptive(72.dp), Modifier.padding(8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            items(sizes.systems) { (name, code) -> OutlinedButton(onClick = { write(code) }) { Text(name, style = MaterialTheme.typography.titleMedium, maxLines = 1) } }
        }
    }
}

/** A class's units, as buttons that write them, on a tab per system. */
@Composable
private fun MeasureUnits(measure: Measure, write: (String) -> Unit, columns: GridCells) {
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
        LazyVerticalGrid(columns, Modifier.padding(8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
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

/** Every currency, the most used first, and a search field above them (typed with the phone's
 *  keyboard): a tap writes its code; ⏎ on the phone's keyboard, the first one found. Wide
 *  (landscape), while the phone's keyboard is up, only a line is left: the ones found beside the
 *  field, as chips. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CurrenciesPanel(write: (String) -> Unit, wide: Boolean) {
    val currencies = loaded { Core.currencies() } ?: return
    // Kept across a rotation.
    var query by rememberSaveable { mutableStateOf("") }
    val found = remember(currencies, query) { search(currencies, query) }
    val keyboard = LocalSoftwareKeyboardController.current
    // The phone's keyboard away before the panel closes, the request's keys back.
    val pick = { c: Currency -> keyboard?.hide(); write(c.code) }
    val field = @Composable { modifier: Modifier ->
        OutlinedTextField(
            query,
            onValueChange = { query = it },
            modifier = modifier.padding(horizontal = 8.dp, vertical = 4.dp),
            placeholder = { Text("Search: euro, yen, ₹, CHF, Brazil…", maxLines = 1, overflow = TextOverflow.Ellipsis) },
            singleLine = true,
            leadingIcon = { Icon(SearchIcon, contentDescription = null) },
            trailingIcon = {
                if (query.isNotEmpty()) IconButton(onClick = { query = "" }) { Icon(CloseIcon, contentDescription = "Clear the search") }
            },
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { found.firstOrNull()?.let(pick) ?: keyboard?.hide() }),
        )
    }
    val none = "No currency matches “${query.trim()}”."
    // The field always the first in the same row: drawn again elsewhere, it would lose the focus,
    // and the phone's keyboard close.
    val inLine = wide && WindowInsets.isImeVisible
    // Another search: the list from its beginning, the best match first.
    val list = rememberLazyListState()
    LaunchedEffect(query) { list.scrollToItem(0) }
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            field(if (inLine) Modifier.width(ListWidth * 1.4f) else Modifier.fillMaxWidth())
            if (inLine) {
                if (found.isEmpty()) Text(none, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                LazyRow(state = list, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    items(found, key = { it.code }) { c ->
                        val country = countriesMatched(c, query).firstOrNull()?.let { " ($it)" }.orEmpty()
                        AssistChip(onClick = { pick(c) }, label = { Text("${c.code}  ${c.name}$country") })
                    }
                }
            }
        }
        if (!inLine) {
            if (found.isEmpty()) Text(none, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(16.dp))
            LazyColumn(state = list) {
                items(found, key = { it.code }) { c ->
                    // Found by a country: it before the symbols (`Italy · €`).
                    val countries = countriesMatched(c, query)
                    val shown = if (countries.size > 3) countries.take(3) + "…" else countries
                    Line("${c.code}  ${c.name}", (shown + c.symbols.joinToString(" ")).filter { it.isNotEmpty() }.joinToString(" · ")) { pick(c) }
                }
            }
        }
    }
}

/**
 * The currencies matching `query`, ignoring case and accents: its code first (`ch` → CHF), then a
 * word of its name beginning with it (`yen`, `franc` → the Swiss and the CFA francs), then a word of
 * a country using it (`italy` → the euro, `usa` → the US dollar, `franc` → the euro too, for
 * France), then its name, a country, a symbol (`₹`, `kr`) or the code containing it anywhere. Within each, the order as given (the most used first).
 * An empty query: all of them.
 */
fun search(currencies: List<Currency>, query: String): List<Currency> {
    val q = plain(query.trim())
    if (q.isEmpty()) return currencies
    fun rank(c: Currency): Int? {
        val name = plain(c.name)
        return when {
            plain(c.code).startsWith(q) -> 0
            startsAWord(name, q) -> 1
            countriesFound(c, q, anywhere = false).isNotEmpty() -> 2
            name.contains(q) || plain(c.code).contains(q) || c.symbols.any { plain(it).contains(q) } || countriesFound(c, q, anywhere = true).isNotEmpty() -> 3
            else -> null
        }
    }
    return currencies.mapNotNull { c -> rank(c)?.let { it to c } }.sortedBy { it.first }.map { it.second }
}

/** The countries a search found `c` by (`united` → United States, for the dollar): none when its
 *  own name or code matches, as those say it already. Shown in its row, so the match is clear. */
fun countriesMatched(c: Currency, query: String): List<String> {
    val q = plain(query.trim())
    if (q.isEmpty() || plain(c.code).contains(q) || plain(c.name).contains(q)) return emptyList()
    return countriesFound(c, q, anywhere = false).ifEmpty { countriesFound(c, q, anywhere = true) }
}

/** The countries of `c` (by name, or by another name: `USA` is the United States) where `q` begins
 *  a word, or with `anywhere` is anywhere in it. */
private fun countriesFound(c: Currency, q: String, anywhere: Boolean): List<String> {
    val found = { text: String -> if (anywhere) plain(text).contains(q) else startsAWord(plain(text), q) }
    val byName = c.countries.filter(found)
    // An alias stands for its country, which isn't known here: the alias itself is shown.
    return byName.ifEmpty { c.aliases.filter(found) }
}

private fun startsAWord(text: String, q: String): Boolean = text.startsWith(q) || text.split(' ', '-', '(').any { it.startsWith(q) }

/** Lower case, without accents: `Réal` → `real`, so the search finds it either way. */
private fun plain(text: String): String =
    java.text.Normalizer.normalize(text.lowercase(), java.text.Normalizer.Form.NFD).replace(Regex("\\p{Mn}+"), "")

/** The Trivia's sections; a section opened: its values now; a value tapped: its details, as the
 *  TUI's popup (the description, then each line), and a button that writes it into the request.
 *  Wide: the three side by side, the section and the value chosen highlighted. */
@Composable
private fun TriviaPanel(
    opened: String?,
    detail: String?,
    open: (String) -> Unit,
    show: (String) -> Unit,
    write: (String) -> Unit,
    openLink: (String) -> Unit,
    wide: Boolean,
) {
    val sections = loaded { Core.trivia() } ?: return
    val section = sections.firstOrNull { it.name == opened }
    val entry = section?.entries?.firstOrNull { it.name == detail }
    val sectionList = @Composable { modifier: Modifier ->
        LazyColumn(modifier) {
            items(sections) { s -> Line(s.name, s.entries.take(3).joinToString(" · ") { it.name }, icon = triviaIcon(s.name), selected = s.name == opened) { open(s.name) } }
        }
    }
    val entryList = @Composable { s: TriviaSection, modifier: Modifier ->
        LazyColumn(modifier) {
            // What the value is, then the value: `Distance from the Earth now: 139,915,713 km (0.9353 au)`.
            items(s.entries) { e ->
                Line(e.name, if (e.value.isEmpty()) e.short else "${e.short}: ${e.value}", symbol = e.symbol, detailLines = 2, selected = e.name == detail) { show(e.name) }
            }
        }
    }
    when {
        // The lists at their widths, the details at least `DetailWidth` (the rest of the width, if
        // more): when they don't fit (a phone), the row scrolls sideways, to the details when a value
        // is opened and back to the sections when it's closed.
        wide -> BoxWithConstraints(Modifier.fillMaxSize()) {
            val sectionsWidth = ListWidth * 0.8f
            val entriesWidth = ListWidth * 1.3f
            val detailWidth = maxOf(DetailWidth, maxWidth - sectionsWidth - entriesWidth - 2.dp)
            val scroll = rememberScrollState()
            LaunchedEffect(detail) {
                // After the layout with the details (or without), which sets how far it scrolls.
                withFrameNanos {}
                scroll.animateScrollTo(if (detail != null) scroll.maxValue else 0)
            }
            Row(Modifier.fillMaxHeight().horizontalScroll(scroll)) {
                sectionList(Modifier.width(sectionsWidth).fillMaxHeight())
                VerticalDivider()
                if (section != null) {
                    entryList(section, Modifier.width(entriesWidth).fillMaxHeight())
                    VerticalDivider()
                }
                if (entry != null) {
                    Box(Modifier.width(detailWidth).fillMaxHeight()) {
                        TriviaDetail(entry, write, openLink)
                    }
                }
            }
        }
        section == null -> sectionList(Modifier)
        entry == null -> entryList(section, Modifier)
        else -> TriviaDetail(entry, write, openLink)
    }
}

/** A Trivia value in full, as the TUI's popup: the description, then every line (the value among
 *  them), each measure of a line a chip that writes it (`172.4685 au`); a link opens inside the app.
 *  The request gets a value only from a chip or the button. */
@Composable
private fun TriviaDetail(e: TriviaEntry, write: (String) -> Unit, openLink: (String) -> Unit) {
    Column(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.weight(1f).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (e.description.isNotEmpty()) item { Text(e.description, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(top = 8.dp)) }
            items(e.details) { line ->
                val link = line.text.startsWith("https://")
                Column(if (link) Modifier.clickable(role = Role.Button, onClickLabel = "Open") { openLink(line.text) } else Modifier) {
                    Text(line.label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(line.text, style = MaterialTheme.typography.bodyMedium, color = if (link) MaterialTheme.colorScheme.primary else Color.Unspecified)
                    // Each measure, to pick: the distance in km, au or ly, a speed in km/s, km/h or mph…
                    if (line.measures.size > 1) {
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            line.measures.forEach { (shown, written) -> AssistChip(onClick = { write(written) }, label = { Text(shown) }) }
                        }
                    }
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

/** A row of a panel's list: an icon (a unit class's, a Trivia section's) or a symbol (`♃`, a Trivia
 *  value's), a title, a line below it, a tap. */
@Composable
private fun Line(
    title: String,
    detail: String,
    symbol: String = "",
    icon: ImageVector? = null,
    detailLines: Int = 1,
    selected: Boolean = false,
    onClick: () -> Unit,
) {
    // The item chosen (wide, its next level beside it): highlighted.
    val highlight = if (selected) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent
    Row(Modifier.fillMaxWidth().background(highlight).clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        if (icon != null) {
            Box(Modifier.width(44.dp), contentAlignment = Alignment.CenterStart) {
                Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
            }
        } else if (symbol.isNotEmpty()) {
            Text(
                symbol,
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.primary,
                textAlign = TextAlign.Center,
                maxLines = 1,
                modifier = Modifier.width(44.dp).clearAndSetSemantics {},
            )
        }
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            if (detail.isNotEmpty()) {
                Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = detailLines, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}
