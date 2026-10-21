package io.lucapf.aequalis

import androidx.compose.animation.core.CubicBezierEasing
import org.junit.Assert.assertEquals
import org.junit.Test

/** The splash's timing, as the SVG's `keyTimes` (iconset/splash/aequalis-splash-animated.svg). */
class SplashTest {
    private val times = floatArrayOf(0f, 0.42f, 0.58f, 1f)
    private val values = floatArrayOf(0f, 0f, 1f, 1f)

    @Test
    fun heldThenRisingThenHeld() {
        assertEquals(0f, keyframes(0.2f, times, values), 1e-6f)
        assertEquals(0.5f, keyframes(0.5f, times, values), 1e-6f)
        assertEquals(1f, keyframes(0.9f, times, values), 1e-6f)
        assertEquals(1f, keyframes(1.5f, times, values), 1e-6f)
    }

    @Test
    fun easedBetweenTwoTimes() {
        val slow = CubicBezierEasing(0.4f, 0f, 0.9f, 0.65f)
        val half = keyframes(0.27f, floatArrayOf(0f, 0.54f, 1f), floatArrayOf(1f, 0f, 0f), listOf(slow))
        // The pen slow at first: less than half of the outline drawn at half its time.
        assert(1f - half < 0.5f) { "drawn: ${1f - half}" }
    }
}
