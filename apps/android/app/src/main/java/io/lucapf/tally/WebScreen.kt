package io.lucapf.tally

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView

/**
 * A web page inside the app (a Trivia value's Wikipedia page), so reading it doesn't leave Tally:
 * Android's own WebView, no library. Back goes back through the pages followed, then closes;
 * "Open in browser" hands the page to the default browser.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun WebScreen(url: String, close: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    var web by remember { mutableStateOf<WebView?>(null) }
    var title by remember { mutableStateOf("") }
    BackHandler { web?.takeIf { it.canGoBack() }?.goBack() ?: close() }
    Column(modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = close) { Icon(BackIcon, contentDescription = "Back to Tally") }
            Text(title, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            TextButton(onClick = {
                // The page shown now (a link followed, maybe), in the default browser; none: nothing.
                val shown = web?.url ?: url
                try {
                    context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(shown)))
                } catch (_: ActivityNotFoundException) {
                }
            }) { Text("Open in browser") }
        }
        AndroidView(
            factory = { ctx ->
                WebView(ctx).apply {
                    // Wikipedia needs JavaScript for its menus and images; pages stay in this view.
                    settings.javaScriptEnabled = true
                    webViewClient = object : WebViewClient() {
                        override fun onPageFinished(view: WebView, address: String) {
                            title = view.title.orEmpty()
                        }
                    }
                    loadUrl(url)
                    web = this
                }
            },
            modifier = Modifier.weight(1f).navigationBarsPadding(),
        )
    }
}
