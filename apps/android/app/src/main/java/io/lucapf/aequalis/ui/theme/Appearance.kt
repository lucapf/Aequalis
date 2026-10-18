package io.lucapf.aequalis.ui.theme

import android.content.Context
import android.os.Build
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color

/** Light or dark: as the phone is set, or always one of them. */
enum class DarkMode(val label: String) { System("System"), Light("Light"), Dark("Dark") }

/** The app's colours: from the phone's wallpaper (Android 12+), the classic ones, or a skin with a
 *  gradient behind everything. Each has a light and a dark version. */
enum class Skin(val label: String) {
    Device("Device"),
    Classic("Classic"),
    Ocean("Ocean"),
    Sunset("Sunset"),
    Forest("Forest"),
    Lavender("Lavender"),
}

/**
 * How the app looks: the mode and the skin, chosen in Settings and kept on the phone (the app's
 * own preferences: the core has no colours). Compose state, so the screen is drawn again at once
 * when one changes.
 */
object Appearance {
    private const val FILE = "appearance"
    var mode by mutableStateOf(DarkMode.System)
        private set
    var skin by mutableStateOf(Skin.Device)
        private set
    private var context: Context? = null

    /** The choices saved, once, at start. */
    fun load(context: Context) {
        this.context = context.applicationContext
        val prefs = context.getSharedPreferences(FILE, Context.MODE_PRIVATE)
        mode = DarkMode.entries.firstOrNull { it.name == prefs.getString("mode", null) } ?: DarkMode.System
        skin = Skin.entries.firstOrNull { it.name == prefs.getString("skin", null) } ?: Skin.Device
    }

    fun choose(mode: DarkMode = this.mode, skin: Skin = this.skin) {
        this.mode = mode
        this.skin = skin
        context?.getSharedPreferences(FILE, Context.MODE_PRIVATE)?.edit()?.putString("mode", mode.name)?.putString("skin", skin.name)?.apply()
    }
}

/** A skin's colours: the accent (⏎, links, the cursor), the operator keys, the other keys, and
 *  the gradient behind the screen (top to bottom). */
private class Palette(val accent: Color, val onAccent: Color, val operator: Color, val onOperator: Color, val key: Color, val action: Color, val top: Color, val bottom: Color)

private fun palette(skin: Skin, dark: Boolean): Palette? = when (skin) {
    Skin.Device, Skin.Classic -> null
    Skin.Ocean -> if (dark) {
        Palette(Color(0xFF4FC3F7), Color(0xFF00283A), Color(0xFF0B4F6C), Color(0xFFCDEBFF), Color(0xFF16384A), Color(0xFF102C3B), Color(0xFF001521), Color(0xFF00344A))
    } else {
        Palette(Color(0xFF00658F), Color.White, Color(0xFFB9E6FF), Color(0xFF001E2E), Color(0xFFF2FBFF), Color(0xFFDDF1FA), Color(0xFFE6F7FF), Color(0xFF9FD9F5))
    }
    Skin.Sunset -> if (dark) {
        Palette(Color(0xFFFFB68D), Color(0xFF4A1F00), Color(0xFF6E2A3A), Color(0xFFFFD9DF), Color(0xFF3A1D28), Color(0xFF2E1720), Color(0xFF1E0B14), Color(0xFF4A1A2A))
    } else {
        Palette(Color(0xFFB8480E), Color.White, Color(0xFFFFD3C2), Color(0xFF3A1100), Color(0xFFFFF6F1), Color(0xFFFFE6DA), Color(0xFFFFF0E3), Color(0xFFFFB3C7))
    }
    Skin.Forest -> if (dark) {
        Palette(Color(0xFF8BD88F), Color(0xFF00390F), Color(0xFF235A2C), Color(0xFFCFEFCB), Color(0xFF1C3320), Color(0xFF16291A), Color(0xFF07150A), Color(0xFF173A1F))
    } else {
        Palette(Color(0xFF2E6B30), Color.White, Color(0xFFC6EBC2), Color(0xFF002106), Color(0xFFF6FBF3), Color(0xFFE2F1DD), Color(0xFFEEF8E9), Color(0xFFB7DDB0))
    }
    Skin.Lavender -> if (dark) {
        Palette(Color(0xFFD0BCFF), Color(0xFF381E72), Color(0xFF4F378B), Color(0xFFEADDFF), Color(0xFF2E2642), Color(0xFF241E35), Color(0xFF130D24), Color(0xFF2E2252))
    } else {
        Palette(Color(0xFF6750A4), Color.White, Color(0xFFE5DBFF), Color(0xFF21005D), Color(0xFFFBF8FF), Color(0xFFEDE6FA), Color(0xFFF6F1FF), Color(0xFFD3C2F7))
    }
}

/** The colour scheme for `skin`, light or dark. A gradient skin leaves the surfaces transparent,
 *  so its background shows through; the keyboard and the panels get a translucent tint. */
fun colorScheme(context: Context, skin: Skin, dark: Boolean): ColorScheme {
    val base = if (dark) darkColorScheme() else lightColorScheme()
    return when (skin) {
        // The wallpaper's colours, from Android 12; before, the classic ones.
        Skin.Device, Skin.Classic -> if (skin == Skin.Device && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            if (dark) androidx.compose.material3.dynamicDarkColorScheme(context) else androidx.compose.material3.dynamicLightColorScheme(context)
        } else if (dark) {
            darkColorScheme(primary = Purple80, secondary = PurpleGrey80, tertiary = Pink80)
        } else {
            lightColorScheme(primary = Purple40, secondary = PurpleGrey40, tertiary = Pink40)
        }
        else -> {
            val p = palette(skin, dark)!!
            base.copy(
                primary = p.accent,
                onPrimary = p.onAccent,
                secondaryContainer = p.operator,
                onSecondaryContainer = p.onOperator,
                surfaceContainerHighest = p.key,
                surfaceContainerHigh = p.action,
                surfaceContainer = (if (dark) Color.Black else Color.White).copy(alpha = if (dark) 0.35f else 0.5f),
                surface = Color.Transparent,
                background = Color.Transparent,
            )
        }
    }
}

/** What is drawn behind the whole screen: a skin's gradient, else the scheme's background. */
fun backdrop(skin: Skin, dark: Boolean, scheme: ColorScheme): Brush =
    palette(skin, dark)?.let { Brush.verticalGradient(listOf(it.top, it.bottom)) } ?: Brush.verticalGradient(listOf(scheme.background, scheme.background))
