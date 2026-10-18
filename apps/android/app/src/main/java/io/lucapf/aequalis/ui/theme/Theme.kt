package io.lucapf.aequalis.ui.theme

import android.app.Activity
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

/** Whether the app is dark now: the mode chosen in Settings, or the phone's. */
@Composable
fun isDark(): Boolean = when (Appearance.mode) {
    DarkMode.System -> isSystemInDarkTheme()
    DarkMode.Light -> false
    DarkMode.Dark -> true
}

/** Aequalis's theme: the skin and the mode chosen (`Appearance`), the skin's background behind
 *  everything, and the status bar's icons dark on a light screen, light on a dark one. */
@Composable
fun AequalisTheme(content: @Composable () -> Unit) {
    val dark = isDark()
    val scheme = colorScheme(LocalContext.current, Appearance.skin, dark)
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            WindowCompat.getInsetsController(window, view).run {
                isAppearanceLightStatusBars = !dark
                isAppearanceLightNavigationBars = !dark
            }
        }
    }
    MaterialTheme(colorScheme = scheme, typography = Typography) {
        Box(Modifier.fillMaxSize().background(backdrop(Appearance.skin, dark, scheme))) { content() }
    }
}
