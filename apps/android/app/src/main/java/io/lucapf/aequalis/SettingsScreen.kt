package io.lucapf.aequalis

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.material3.ButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.lucapf.aequalis.ui.theme.Appearance
import io.lucapf.aequalis.ui.theme.DarkMode
import io.lucapf.aequalis.ui.theme.Skin
import io.lucapf.aequalis.ui.theme.backdrop
import io.lucapf.aequalis.ui.theme.colorScheme
import io.lucapf.aequalis.ui.theme.isDark
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** The width of the keys' column: the values, and their lists, start after it (as the widget's). */
private val KEY_WIDTH = 140.dp

/**
 * Every setting, laid out as the Omarchy widget's (omarchy-widget/Panel.qml): a row each, its key
 * (in the accent when changed; tapped, what it does) and its value. A yes/no changes with a tap;
 * a choice and the currency open their list under the row, the currency's filtered as you type;
 * the others are typed (Done saves). `default` puts back the default. Only the settings most
 * people use until Advanced; then Reset (tapped twice) and Sync. Online or offline, and the age of
 * the last sync, under the title.
 */
@Composable
fun SettingsScreen(close: () -> Unit, openAbout: () -> Unit, modifier: Modifier = Modifier) {
    BackHandler(onBack = close)
    val scope = rememberCoroutineScope()
    // Read again after each change: the core normalises the value (`ON` is `true`).
    var settings by remember { mutableStateOf(Core.settings()) }
    var online by remember { mutableStateOf<Online?>(null) }
    var message by rememberSaveable { mutableStateOf<String?>(null) }
    var advanced by rememberSaveable { mutableStateOf(false) }
    var picking by rememberSaveable { mutableStateOf<String?>(null) }
    var confirmReset by rememberSaveable { mutableStateOf(false) }
    var syncing by remember { mutableStateOf(false) }
    // The running sync's step, and how the last one ended (true: everything synced).
    var step by remember { mutableStateOf("") }
    var outcome by rememberSaveable { mutableStateOf<Boolean?>(null) }
    val refreshOnline = { scope.launch { online = withContext(Dispatchers.Default) { Core.online() } } }
    LaunchedEffect(Unit) { refreshOnline() }
    val change = { key: String, value: String ->
        message = Core.change(key, value)
        settings = Core.settings()
        picking = null
        // The offline setting is the status's too.
        refreshOnline()
        Unit
    }
    val reset = {
        if (!confirmReset) {
            confirmReset = true
            message = "Tap again to reset everything"
        } else {
            confirmReset = false
            message = Core.resetSettings() ?: "Every setting is back to its default"
            settings = Core.settings()
            refreshOnline()
        }
        Unit
    }
    val sync = {
        if (!syncing) {
            syncing = true
            message = null
            outcome = null
            SyncProgress.start()
            scope.launch {
                // What it is doing, asked as it goes: downloading, checking, installing.
                val steps = launch {
                    while (true) {
                        step = withContext(Dispatchers.IO) { Core.syncStep() }
                        delay(200)
                    }
                }
                val synced = withContext(Dispatchers.IO) { Core.download(auto = false) }
                steps.cancel()
                step = ""
                message = synced.text.ifBlank { null }
                outcome = !synced.failed
                SyncProgress.stop()
                syncing = false
                refreshOnline()
            }
        }
        Unit
    }
    val shown = settings.filter { advanced || !it.advanced }
    val hiddenChanged = settings.count { it.advanced && it.value != it.default }
    // Above the keyboard and the navigation bar, whichever is taller (the app draws edge to edge).
    Column(modifier.fillMaxSize().windowInsetsPadding(WindowInsets.ime.union(WindowInsets.navigationBars))) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = close) { Icon(BackIcon, contentDescription = "Back") }
            Column(Modifier.weight(1f)) {
                Text("Settings", style = MaterialTheme.typography.titleLarge)
                online?.let {
                    Text(
                        "● " + statusText(it),
                        style = MaterialTheme.typography.bodySmall,
                        color = if (it.status == "online") MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            TextButton(onClick = openAbout) {
                Icon(InfoIcon, contentDescription = null, modifier = Modifier.padding(end = 6.dp).size(18.dp))
                Text("About")
            }
        }
        LazyColumn(contentPadding = PaddingValues(vertical = 8.dp)) {
            item(key = "appearance") { AppearanceRows(picking) { picking = if (picking == it) null else it } }
            items(shown, key = { it.key }) { s ->
                SettingRow(s, open = picking == s.key, toggle = { picking = if (picking == s.key) null else s.key }, change = change)
            }
            item(key = "buttons") {
                FlowRow(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { advanced = !advanced }) {
                        Text(if (advanced) "Fewer" else if (hiddenChanged > 0) "Advanced ($hiddenChanged changed)" else "Advanced")
                    }
                    OutlinedButton(onClick = reset) { Text(if (confirmReset) "Reset: sure?" else "Reset") }
                    SyncButton(syncing, outcome, sync)
                }
                // While syncing, what it is doing (and the speed while downloading); then what was
                // synced and what failed.
                (if (syncing) syncStatus(step) else message)?.let {
                    Text(it, color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(horizontal = 16.dp))
                }
            }
        }
    }
}

/** What a running sync says under the buttons: its step, and the speed while downloading
 *  (`downloading the city archive · 400 kB/s`). */
private fun syncStatus(step: String): String = when {
    step.isEmpty() -> "starting"
    step.startsWith("downloading") -> "$step · ${speedText(SyncProgress.speed)}"
    else -> step
}

/** A sync that ended well (green) or with an error (orange). */
private val SYNCED = Color(0xFF2E7D32)
private val SYNC_FAILED = Color(0xFFE65100)

/** Sync; while syncing, **Syncing**, filled in the accent colour as far as the big download is
 *  (`SyncProgress.fraction`), or a band sweeping across while there is no percentage (checking,
 *  installing). At its end green, **Synced**, or orange, **Sync failed**, until the next one. A tap
 *  while syncing does nothing (`sync` says so), the label stays readable. */
@Composable
private fun SyncButton(syncing: Boolean, outcome: Boolean?, sync: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary.copy(alpha = 0.35f)
    val fraction = SyncProgress.fraction
    // The sweep runs only while a sync has no percentage to show.
    val sweep = if (syncing && fraction == null) {
        rememberInfiniteTransition(label = "sync").animateFloat(
            initialValue = -0.3f,
            targetValue = 1f,
            animationSpec = infiniteRepeatable(tween(1200, easing = LinearEasing)),
            label = "sweep",
        ).value
    } else {
        0f
    }
    val fill = Modifier.clip(ButtonDefaults.outlinedShape).drawBehind {
        if (!syncing) return@drawBehind
        when (fraction) {
            null -> drawRect(accent, topLeft = Offset(size.width * sweep, 0f), size = Size(size.width * 0.3f, size.height))
            else -> drawRect(accent, size = Size(size.width * fraction, size.height))
        }
    }
    val ended = if (syncing) null else outcome
    val colors = when (ended) {
        true -> ButtonDefaults.outlinedButtonColors(containerColor = SYNCED, contentColor = Color.White)
        false -> ButtonDefaults.outlinedButtonColors(containerColor = SYNC_FAILED, contentColor = Color.White)
        null -> ButtonDefaults.outlinedButtonColors()
    }
    val label = when {
        syncing -> "Syncing"
        ended == true -> "Synced"
        ended == false -> "Sync failed"
        else -> "Sync"
    }
    OutlinedButton(onClick = sync, modifier = fill, colors = colors) { Text(label) }
}

/** `online (2h old)`, `offline (forced, 2d old)`, `online (just synced)`, `never synced`. */
internal fun statusText(online: Online): String {
    val age = when (online.synced) {
        null -> "never synced"
        "now" -> "just synced"
        else -> "${online.synced} old"
    }
    return when (online.status) {
        "offline (forced)" -> "offline (forced, $age)"
        "not synced" -> age
        else -> "${online.status} ($age)"
    }
}

/** A value as shown: `auto (AED)` where auto stands for something on this phone. */
internal fun shownValue(value: String, auto: String?): String = if (value == "auto" && auto != null) "auto ($auto)" else value

/** The list of a choice or of the currency: (value, label), `auto` as it stands (`auto (km)`), the
 *  currencies with their name; filtered by what is typed, at most 8. */
internal fun pickChoices(s: Setting, typed: String): List<Pair<String, String>> {
    val all = if (s.currencies.isNotEmpty()) {
        listOf("auto" to shownValue("auto", s.auto)) + s.currencies.map { (code, name) -> code to "$code  $name" }
    } else {
        s.options.map { it to shownValue(it, s.auto) }
    }
    val wanted = typed.trim().lowercase()
    return all.filter { it.second.lowercase().contains(wanted) }.take(8)
}

/** The app's look (its own preferences, `Appearance`), as two more rows: light or dark, and the
 *  skin, each skin shown by a swatch of its background and accent. */
@Composable
private fun AppearanceRows(picking: String?, toggle: (String) -> Unit) {
    val context = LocalContext.current
    val dark = isDark()
    KeyRow("appearance", Appearance.mode != DarkMode.System, "Light or dark, or as the phone is", onDefault = { Appearance.choose(mode = DarkMode.System) }, value = {
        ValueButton(Appearance.mode.label + "  ▾") { toggle("appearance") }
    }) {
        if (picking == "appearance") {
            PickList(DarkMode.entries.map { it.name to it.label }, Appearance.mode.name) { name -> Appearance.choose(mode = DarkMode.valueOf(name)); toggle("appearance") }
        }
    }
    KeyRow("skin", Appearance.skin != Skin.Device, "The colours", onDefault = { Appearance.choose(skin = Skin.Device) }, value = {
        ValueButton(Appearance.skin.label + "  ▾") { toggle("skin") }
    }) {
        if (picking == "skin") {
            PickList(Skin.entries.map { it.name to it.label }, Appearance.skin.name, leading = { name ->
                val skin = Skin.valueOf(name)
                val scheme = colorScheme(context, skin, dark)
                Box(Modifier.padding(end = 8.dp).size(16.dp).clip(CircleShape).background(backdrop(skin, dark, scheme)).border(4.dp, scheme.primary, CircleShape).clearAndSetSemantics {})
            }) { name -> Appearance.choose(skin = Skin.valueOf(name)); toggle("skin") }
        }
    }
}

@Composable
private fun SettingRow(s: Setting, open: Boolean, toggle: () -> Unit, change: (String, String) -> Unit) {
    val typed = s.kind == "text" && s.currencies.isEmpty()
    KeyRow(s.key, s.value != s.default, "${s.about} (default: ${s.default})", onDefault = { change(s.key, "") }, value = {
        when {
            typed -> TypedValue(s.value) { change(s.key, it) }
            s.kind == "bool" -> ValueButton(s.value) { change(s.key, if (s.value == "true") "false" else "true") }
            else -> ValueButton(shownValue(s.value, s.auto) + "  ▾", toggle)
        }
    }) {
        if (open) {
            var filter by rememberSaveable { mutableStateOf("") }
            val choices = pickChoices(s, filter)
            val pick = { value: String -> change(s.key, value) }
            if (s.currencies.isNotEmpty()) {
                val focus = remember { FocusRequester() }
                Field(filter, { filter = it }, Modifier.padding(start = KEY_WIDTH, end = 16.dp).fillMaxWidth().focusRequester(focus), placeholder = "type to filter") {
                    choices.firstOrNull()?.let { pick(it.first) }
                }
                LaunchedEffect(Unit) { focus.requestFocus() }
            }
            PickList(choices, s.value, pick = pick)
        }
    }
}

/** A row: the key (the accent when changed; tapped, `about`), its value, `default` when changed;
 *  then what opens under it. */
@Composable
private fun KeyRow(key: String, changed: Boolean, about: String, onDefault: () -> Unit, value: @Composable RowScope.() -> Unit, below: @Composable () -> Unit) {
    var showAbout by rememberSaveable { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().padding(vertical = 2.dp)) {
        Row(Modifier.padding(start = 16.dp, end = 8.dp).heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                key,
                color = if (changed) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.width(KEY_WIDTH - 16.dp).clickable { showAbout = !showAbout },
            )
            Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, content = value)
            if (changed) TextButton(onClick = onDefault) { Text("default") }
        }
        if (showAbout) {
            Text(about, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 4.dp))
        }
        below()
    }
}

@Composable
private fun ValueButton(text: String, onClick: () -> Unit) {
    OutlinedButton(onClick = onClick, contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
        Text(text, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** A typed value (an address, a date pattern, a duration): Done saves it. */
@Composable
private fun TypedValue(value: String, save: (String) -> Unit) {
    var draft by rememberSaveable(value) { mutableStateOf(value) }
    Field(draft, { draft = it }, Modifier.fillMaxWidth()) { if (draft != value) save(draft) }
}

/** A one-line field as compact as the value buttons; `done`: the keyboard's Done. */
@Composable
private fun Field(text: String, onChange: (String) -> Unit, modifier: Modifier = Modifier, placeholder: String = "", done: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    BasicTextField(
        text,
        onChange,
        modifier,
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyMedium.copy(color = colors.onSurface),
        cursorBrush = SolidColor(colors.primary),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { done() }),
        decorationBox = { inner ->
            Box(Modifier.border(1.dp, colors.outline, RoundedCornerShape(8.dp)).padding(horizontal = 12.dp, vertical = 10.dp)) {
                if (text.isEmpty()) Text(placeholder, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                inner()
            }
        },
    )
}

/** The list under a row, from the values' column: (value, label), the current one outlined. */
@Composable
private fun PickList(choices: List<Pair<String, String>>, current: String, leading: (@Composable (String) -> Unit)? = null, pick: (String) -> Unit) {
    Column(Modifier.padding(start = KEY_WIDTH - 12.dp, end = 16.dp, bottom = 4.dp)) {
        choices.forEach { (value, label) ->
            val chosen = value == current
            val shape = RoundedCornerShape(8.dp)
            Row(
                Modifier.fillMaxWidth().clip(shape).then(if (chosen) Modifier.border(1.dp, MaterialTheme.colorScheme.primary, shape) else Modifier)
                    .clickable { pick(value) }.padding(horizontal = 12.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                leading?.invoke(value)
                Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis, color = if (chosen) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface)
            }
        }
    }
}
