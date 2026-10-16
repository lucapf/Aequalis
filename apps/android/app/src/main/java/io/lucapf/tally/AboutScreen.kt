package io.lucapf.tally

import android.content.Intent
import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Where Tally is installed from, for each system: (system, address). Android: the Play Store's
 *  page, once the app is published there. */
private val INSTALL = listOf(
    "Mac" to "https://github.com/lucapf/tally/blob/master/docs/install_mac.md",
    "Linux" to "https://github.com/lucapf/tally/releases/latest",
    "Source code" to "https://github.com/lucapf/tally",
)

/** The Play Store's page of the app: null until it is published there. */
private val PLAY_STORE: String? = null

/** What Share sends: the install links, to give Tally to someone. */
private fun installText(tagline: String): String {
    val links = (INSTALL + listOfNotNull(PLAY_STORE?.let { "Android" to it })).joinToString("\n") { (system, link) -> "$system: $link" }
    return "Tally, $tagline.\n$links"
}

/** About Tally: the version, the online data and its sources (with their credits), how to
 *  install Tally on the other systems, and Share for those links. */
@Composable
fun AboutScreen(close: () -> Unit, modifier: Modifier = Modifier) {
    BackHandler(onBack = close)
    val context = LocalContext.current
    val about by produceState<About?>(null) { value = withContext(Dispatchers.Default) { Core.about() } }
    val open = { link: String -> context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(link))) }
    val appVersion = runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull()
    Column(modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = close) { Icon(BackIcon, contentDescription = "Back") }
            Text("About", style = MaterialTheme.typography.titleLarge)
        }
        val a = about ?: return@Column
        LazyColumn(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            item {
                Text(a.name, style = MaterialTheme.typography.headlineMedium)
                Text(a.tagline, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text("App ${appVersion ?: "?"} · core ${a.version}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            items(a.facts) { (label, text) -> Fact(label, text) }
            item { HorizontalDivider() }
            item { Text("Install Tally", style = MaterialTheme.typography.titleMedium) }
            items(INSTALL) { (system, link) -> Fact(system, link, onClick = { open(link) }) }
            item {
                Button(onClick = {
                    val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, installText(a.tagline))
                    context.startActivity(Intent.createChooser(send, null))
                }) {
                    Icon(ShareIcon, contentDescription = null)
                    Text("  Share the install links")
                }
            }
            item { HorizontalDivider() }
            item { Text("Data", style = MaterialTheme.typography.titleMedium) }
            items(a.sources) { (label, text) -> Fact(label, text) }
            // The app's own: its icons (Icons.kt).
            item { Fact("Icons", "Material Icons by Google (https://github.com/google/material-design-icons), Apache License 2.0") }
        }
    }
}

/** A label and its text; with `onClick`, a link (the text in the accent colour). */
@Composable
private fun Fact(label: String, text: String, onClick: (() -> Unit)? = null) {
    Column(if (onClick != null) Modifier.fillMaxWidth().clickable(role = Role.Button, onClickLabel = "Open", onClick = onClick) else Modifier.fillMaxWidth()) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(text, style = MaterialTheme.typography.bodyMedium, color = if (onClick != null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface)
    }
}
