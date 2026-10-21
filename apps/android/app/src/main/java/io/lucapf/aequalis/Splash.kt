package io.lucapf.aequalis

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.PathMeasure
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import kotlinx.coroutines.delay

/** The splash's colours (iconset/README.txt). */
private val GREEN = Color(0xFF1F5B45)
private val IVORY = Color(0xFFF7F0E6)
private val GOLD = Color(0xFFF2B84B)
private val ORANGE = Color(0xFFE8742B)

// The q and the two arrows, in the SVG's 1024 × 1024 (iconset/splash/aequalis-splash-animated.svg).
private const val Q_OUTLINE = "M 468 220 A 260 260 0 0 1 716 402 L 716 220 L 816 220 L 816 750 A 50 50 0 0 1 716 750 L 716 558 A 260 260 0 0 1 468 740 A 260 260 0 0 1 468 220 Z"
private const val Q_HOLE = " M 468 330 A 150 150 0 0 1 468 630 A 150 150 0 0 1 468 330 Z"
private const val ARROW_RIGHT = "M 374 398 L 476 398 L 476 378 Q 476 366 486 372 L 552 414 Q 562 420 552 426 L 486 468 Q 476 474 476 462 L 476 442 L 374 442 A 22 22 0 0 1 374 398 Z"
private const val ARROW_LEFT = "M 562 518 L 460 518 L 460 498 Q 460 486 450 492 L 384 534 Q 374 540 384 546 L 450 588 Q 460 594 460 582 L 460 562 L 562 562 A 22 22 0 0 0 562 518 Z"

/** The SVG's eased steps: the pen slow at first, then faster; the arrows landing. */
private val PEN = CubicBezierEasing(0.4f, 0f, 0.9f, 0.65f)
private val LANDING = CubicBezierEasing(0.2f, 0.9f, 0.3f, 1f)

/** Played once per process: not again on a rotation, nor when Android brings the app back. */
object Splash {
    var played = false
}

/**
 * The splash (iconset/splash/, "A · Draw"): an orange pen traces the q, the fill arrives orange,
 * the two gold arrows fly in from the left and the right, and the q cools to ivory as they land;
 * 1.2 s, then a short fade into the app. A tap skips it. Drawn over the app, which starts beneath.
 */
@Composable
fun SplashScreen(done: () -> Unit) {
    val time = remember { Animatable(0f) }
    val fade = remember { Animatable(1f) }
    val shapes = remember { SplashShapes() }
    LaunchedEffect(Unit) {
        // After the first frames, which compose and draw the app beneath too: the pen then starts
        // on a free main thread, and isn't seen jumping ahead.
        repeat(3) { withFrameNanos {} }
        time.animateTo(1f, tween(1200, easing = LinearEasing))
        delay(150)
        fade.animateTo(0f, tween(200))
        done()
    }
    Canvas(
        Modifier.fillMaxSize().alpha(fade.value).semantics { contentDescription = "Aequalis" }.pointerInput(Unit) { detectTapGestures { done() } },
    ) {
        drawRect(GREEN)
        // The SVG's square, centred, as wide as most of the screen's narrow side.
        val side = minOf(size.width, size.height) * 0.8f
        withTransform({
            translate((size.width - side) / 2, (size.height - side) / 2)
            scale(side / 1024f, side / 1024f, Offset.Zero)
        }) { drawQ(shapes, time.value) }
    }
}

/** The SVG's paths, parsed once. */
private class SplashShapes {
    val fill = path(Q_OUTLINE + Q_HOLE).apply { fillType = PathFillType.EvenOdd }
    val outline = path(Q_OUTLINE)
    val measure = PathMeasure().apply { setPath(outline, false) }
    val right = path(ARROW_RIGHT)
    val left = path(ARROW_LEFT)
    val trace = Path()

    private fun path(d: String) = PathParser().parsePathString(d).toPath()
}

/** The frame at `t` (0 to 1 of the 1.2 s), as the SVG's `animate` elements say. */
private fun DrawScope.drawQ(s: SplashShapes, t: Float) {
    // The fill: appears orange, then cools to ivory.
    val fillAlpha = keyframes(t, floatArrayOf(0f, 0.42f, 0.58f, 1f), floatArrayOf(0f, 0f, 1f, 1f))
    val cooling = keyframes(t, floatArrayOf(0f, 0.58f, 0.85f, 1f), floatArrayOf(0f, 0f, 1f, 1f))
    drawPath(s.fill, lerp(ORANGE, IVORY, cooling), alpha = fillAlpha)
    // The pen: draws the outline, then fades out.
    val drawn = 1f - keyframes(t, floatArrayOf(0f, 0.54f, 1f), floatArrayOf(1f, 0f, 0f), listOf(PEN, LinearEasing))
    val penAlpha = keyframes(t, floatArrayOf(0f, 0.54f, 0.64f, 1f), floatArrayOf(1f, 1f, 0f, 0f))
    if (drawn > 0f && penAlpha > 0f) {
        s.trace.reset()
        s.measure.getSegment(0f, s.measure.length * drawn, s.trace, true)
        drawPath(s.trace, ORANGE, alpha = penAlpha, style = Stroke(width = 32f, cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
    // The arrows: from the left and from the right, a little past their place, then back to it.
    arrow(s.right, t, start = 0.56f, land = 0.88f, from = -800f, past = 22f)
    arrow(s.left, t, start = 0.62f, land = 0.93f, from = 800f, past = -22f)
}

private fun DrawScope.arrow(path: Path, t: Float, start: Float, land: Float, from: Float, past: Float) {
    val x = keyframes(t, floatArrayOf(0f, start, land, 1f), floatArrayOf(from, from, past, 0f), listOf(LinearEasing, LANDING, LANDING))
    val alpha = keyframes(t, floatArrayOf(0f, start, start + 0.02f, 1f), floatArrayOf(0f, 0f, 1f, 1f))
    if (alpha > 0f) translate(left = x) { drawPath(path, GOLD, alpha = alpha) }
}

/** `values` at `times` (0 to 1, rising), eased between each two by `easings` (linear when left
 *  out), as SVG's `keyTimes` and `keySplines`; held after the last. */
internal fun keyframes(t: Float, times: FloatArray, values: FloatArray, easings: List<Easing> = emptyList()): Float {
    val i = (1 until times.size).firstOrNull { t <= times[it] } ?: return values.last()
    val span = times[i] - times[i - 1]
    val local = if (span <= 0f) 1f else (t - times[i - 1]) / span
    val eased = easings.getOrElse(i - 1) { LinearEasing }.transform(local.coerceIn(0f, 1f))
    return values[i - 1] + (values[i] - values[i - 1]) * eased
}
