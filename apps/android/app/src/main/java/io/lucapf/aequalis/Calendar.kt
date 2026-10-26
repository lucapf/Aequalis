package io.lucapf.aequalis

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.ui.input.pointer.pointerInput
import kotlin.math.abs
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import io.lucapf.aequalis.ui.theme.isDark

/** A calendar as the core sends it (`cal`, `cal march 2027`, `cal 2027 de fr`): its months, the
 *  weekdays from the first day of the week, the weekend's columns, today (`2026-10-26`), the
 *  countries of the holidays (code, name) in their colours' order, and the holidays. */
data class Calendar(
    val title: String,
    val year: Boolean,
    val today: String,
    val weekdays: List<String>,
    val weekend: Set<Int>,
    val countries: List<Pair<String, String>>,
    val months: List<CalendarMonth>,
    val holidays: List<CalendarHoliday>,
    /** The requests of the month (the year) before and after: a swipe's. */
    val previous: String = "",
    val next: String = "",
) {
    /** The holidays by day (`month * 32 + day`), in the countries' order: worked out once, not by
     *  each of the year's 365 days drawn. */
    internal val byDay: Map<Int, List<CalendarHoliday>> by lazy {
        holidays.groupBy { it.month * 32 + it.day }.mapValues { (_, on) -> on.sortedBy { it.country } }
    }

    /** Today as `year * 512 + month * 32 + day` (0: not a date). */
    internal val todayKey: Int by lazy {
        today.split('-').mapNotNull { it.toIntOrNull() }.takeIf { it.size == 3 }?.let { (y, m, d) -> y * 512 + m * 32 + d } ?: 0
    }
}

/** A month: its weeks, each with its ISO number and the day of the month in each column (0: none). */
data class CalendarMonth(val title: String, val year: Int, val month: Int, val weeks: List<CalendarWeek>)

data class CalendarWeek(val number: Int, val days: List<Int>)

/** A holiday on a day: its date, day and month, its country (an index into `Calendar.countries`),
 *  its name; the holiday's first day, how many days it lasts and its days for people (`2-3 Dec`). */
data class CalendarHoliday(
    val date: String,
    val day: Int,
    val month: Int,
    val country: Int,
    val name: String,
    val first: String = date,
    val length: Int = 1,
    val days: String = "",
)

/** The holidays of a day, in the countries' order. */
fun Calendar.holidaysOn(month: Int, day: Int): List<CalendarHoliday> = byDay[month * 32 + day].orEmpty()

/** How long a holiday lasts, for people: `1 day`, `2 days · 2-3 Dec`. */
fun lasting(h: CalendarHoliday): String = if (h.length > 1) "${h.length} days · ${h.days}" else "1 day"

/** The month's holidays as listed under it: one a holiday, its days together (`2-3 Dec`). */
fun Calendar.listed(): List<CalendarHoliday> = holidays.distinctBy { Triple(it.country, it.first, it.name) }

/** The countries (indexes) whose holiday a day is, in their order. */
fun Calendar.holidayOf(month: Int, day: Int): List<Int> = holidaysOn(month, day).map { it.country }.distinct()

/** Whether the day is today. */
fun Calendar.isToday(m: CalendarMonth, day: Int): Boolean = todayKey == m.year * 512 + m.month * 32 + day

/** The countries' colours, in order (red, blue, green, purple, teal, as the terminal's), lighter on a
 *  dark screen. */
fun countryColour(index: Int, dark: Boolean): Color {
    val light = listOf(0xFFD32F2F, 0xFF1565C0, 0xFF2E7D32, 0xFF7B1FA2, 0xFF00838F)
    val night = listOf(0xFFEF9A9A, 0xFF90CAF9, 0xFFA5D6A7, 0xFFCE93D8, 0xFF80DEEA)
    return Color((if (dark) night else light)[index % light.size])
}

/** Which way a horizontal swipe of `dx` by `dy` goes, past `limit`: 1 the next month (a swipe to
 *  the left), -1 the one before, 0 no swipe (too short, or more up and down than across). */
fun swipeStep(dx: Float, dy: Float, limit: Float): Int = when {
    abs(dx) < limit || abs(dx) < 2 * abs(dy) -> 0
    dx < 0 -> 1
    else -> -1
}

/** The calendar in the result's place: a month, with its holidays listed under it, or the year as
 *  small months, two a row (four in landscape); under it, the countries in their colours. A touch
 *  says so (`touched`: the keyboard collapses); a swipe left or right asks for the month (the year)
 *  after or before (`step`, with its request). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CalendarView(calendar: Calendar, landscape: Boolean, modifier: Modifier = Modifier, touched: () -> Unit = {}, step: (String) -> Unit = {}) {
    val dark = isDark()
    // Watched, not taken: the days' taps and the answer's scroll still get the gesture.
    val swipe = Modifier.pointerInput(calendar) {
        awaitEachGesture {
            val down = awaitFirstDown(requireUnconsumed = false)
            touched()
            var (dx, dy) = 0f to 0f
            while (true) {
                val change = awaitPointerEvent().changes.firstOrNull { it.id == down.id } ?: break
                if (!change.pressed) break
                val moved = change.position - change.previousPosition
                dx += moved.x
                dy += moved.y
            }
            when (swipeStep(dx, dy, 56.dp.toPx())) {
                1 -> calendar.next.takeIf { it.isNotEmpty() }?.let(step)
                -1 -> calendar.previous.takeIf { it.isNotEmpty() }?.let(step)
            }
        }
    }
    // A day tapped (month, day): its holidays under the grid; tapped again, or a day without, none.
    var selected by remember(calendar) { mutableStateOf<Pair<Int, Int>?>(null) }
    val select = { month: Int, day: Int -> selected = (month to day).takeIf { it != selected && calendar.holidaysOn(month, day).isNotEmpty() } }
    Column(modifier.fillMaxWidth().then(swipe), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (calendar.year) {
            Text(calendar.title, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
            val glyphs = rememberGlyphs(calendar.weekdays)
            calendar.months.chunked(if (landscape) 4 else 2).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    row.forEach { SmallMonth(calendar, it, dark, glyphs, selected, select, Modifier.weight(1f)) }
                    repeat((if (landscape) 4 else 2) - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        } else {
            MonthGrid(calendar, calendar.months.first(), dark, selected, select)
        }
        selected?.let { (month, day) -> HolidayDetail(calendar, month, day, dark) }
        if (calendar.countries.isNotEmpty()) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                calendar.countries.forEachIndexed { i, (code, name) ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Dot(countryColour(i, dark), 10.dp)
                        Text(" $name ($code)", style = MaterialTheme.typography.bodyMedium)
                    }
                }
            }
        }
        if (!calendar.year) {
            calendar.listed().forEach { h ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Dot(countryColour(h.country, dark), 8.dp)
                    Text(h.days.ifEmpty { "${h.day}" }, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(start = 8.dp).width(96.dp))
                    Text(if (h.length > 1) "${h.name} (${h.length} days)" else h.name, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                }
            }
        }
    }
}

/** A day's holidays, tapped: the day, then each country's holiday, its name and how many days. */
@Composable
private fun HolidayDetail(calendar: Calendar, month: Int, day: Int, dark: Boolean) {
    val on = calendar.holidaysOn(month, day)
    val year = calendar.months.firstOrNull { it.month == month }?.year ?: return
    val date = java.time.LocalDate.of(year, month, day)
    val title = date.format(java.time.format.DateTimeFormatter.ofPattern("EEEE d MMMM yyyy", java.util.Locale.getDefault()))
    Surface(color = MaterialTheme.colorScheme.secondaryContainer, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(title.replaceFirstChar { it.titlecase() }, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
            on.forEach { h ->
                Row(verticalAlignment = Alignment.Top) {
                    Box(Modifier.padding(top = 6.dp)) { Dot(countryColour(h.country, dark), 10.dp) }
                    Column(Modifier.padding(start = 8.dp)) {
                        Text(calendar.countries.getOrNull(h.country)?.second.orEmpty(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSecondaryContainer)
                        Text(h.name, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium)
                        Text(lasting(h), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSecondaryContainer)
                    }
                }
            }
        }
    }
}

/** A month's grid: its title, the weekdays, a row a week with its ISO number. Today in a circle; a
 *  holiday in its (first) country's colour, a dot a country under it; the weekend dimmed. */
@Composable
private fun MonthGrid(
    calendar: Calendar,
    m: CalendarMonth,
    dark: Boolean,
    selected: Pair<Int, Int>?,
    select: (Int, Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val dim = MaterialTheme.colorScheme.onSurfaceVariant
    val text = MaterialTheme.typography.bodyLarge
    Column(modifier) {
        Text(m.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 4.dp))
        // The weekdays on a band of the theme's own grey, to read them apart from the days (its top
        // corners round: the weekend's columns go on down from it).
        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(topStart = 6.dp, topEnd = 6.dp)).background(MaterialTheme.colorScheme.surfaceVariant).padding(vertical = 6.dp),
        ) {
            val header = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold)
            Text("Wk", style = header, color = dim.copy(alpha = 0.7f), textAlign = TextAlign.Center, modifier = Modifier.weight(0.8f))
            calendar.weekdays.forEachIndexed { k, name ->
                Text(
                    name,
                    style = header,
                    color = if (k in calendar.weekend) dim.copy(alpha = 0.7f) else MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.weight(1f),
                )
            }
        }
        m.weeks.forEach { w ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("${w.number}", style = MaterialTheme.typography.labelSmall, color = dim.copy(alpha = 0.7f), textAlign = TextAlign.Center, modifier = Modifier.weight(0.8f))
                w.days.forEachIndexed { k, day ->
                    // The weekend's columns on the weekdays' grey, down from their names.
                    val band = if (k in calendar.weekend) Modifier.background(MaterialTheme.colorScheme.surfaceVariant) else Modifier
                    Box(Modifier.weight(1f).height(40.dp).then(band), contentAlignment = Alignment.Center) {
                        if (day > 0) Day(calendar, m, day, k in calendar.weekend, dark, text, selected == (m.month to day)) { select(m.month, day) }
                    }
                }
            }
        }
    }
}

@Composable
private fun Day(
    calendar: Calendar,
    m: CalendarMonth,
    day: Int,
    weekend: Boolean,
    dark: Boolean,
    style: androidx.compose.ui.text.TextStyle,
    chosen: Boolean,
    tap: () -> Unit,
) {
    val on = calendar.holidaysOn(m.month, day)
    val of = on.map { it.country }.distinct()
    val today = calendar.isToday(m, day)
    val colour = when {
        today -> MaterialTheme.colorScheme.onPrimary
        of.isNotEmpty() -> countryColour(of.first(), dark)
        weekend -> MaterialTheme.colorScheme.onSurfaceVariant
        else -> MaterialTheme.colorScheme.onSurface
    }
    val names = on.joinToString(", ") { it.name }
    val described = Modifier.semantics { contentDescription = listOfNotNull("$day", "today".takeIf { today }, names.ifEmpty { null }).joinToString(", ") }
    // A holiday's day: a tap shows its holidays.
    val tappable = if (of.isNotEmpty()) described.clip(RoundedCornerShape(8.dp)).clickable(onClickLabel = "Show the holidays", onClick = tap) else described
    Column(tappable, horizontalAlignment = Alignment.CenterHorizontally) {
        Box(
            Modifier.size(30.dp).clip(CircleShape)
                .let { if (today) it.background(MaterialTheme.colorScheme.primary) else it }
                // The day tapped: ringed in its country's colour.
                .let { if (chosen) it.border(2.dp, countryColour(of.firstOrNull() ?: 0, dark), CircleShape) else it },
            contentAlignment = Alignment.Center,
        ) {
            Text("$day", style = style, color = colour, fontWeight = if (of.isNotEmpty() || today) FontWeight.Bold else FontWeight.Normal)
        }
        if (of.isNotEmpty()) {
            Row(horizontalArrangement = Arrangement.spacedBy(2.dp)) { of.forEach { Dot(countryColour(it, dark), 5.dp) } }
        }
    }
}

/** The texts of the year's small months, measured once for all twelve: the days plain and bold
 *  (by the day, 1 to 31), the weekdays' letters, `Wk` and the week numbers (1 to 53). */
private class Glyphs(
    val plain: List<TextLayoutResult>,
    val bold: List<TextLayoutResult>,
    val weekdays: List<TextLayoutResult>,
    val wk: TextLayoutResult,
    val weeks: List<TextLayoutResult>,
)

@Composable
private fun rememberGlyphs(weekdays: List<String>): Glyphs {
    val measurer = rememberTextMeasurer(cacheSize = 0)
    val style = MaterialTheme.typography.labelSmall
    return remember(measurer, style, weekdays) {
        val header = style.copy(fontWeight = FontWeight.SemiBold)
        Glyphs(
            plain = (0..31).map { measurer.measure("$it", style) },
            bold = (0..31).map { measurer.measure("$it", style.copy(fontWeight = FontWeight.Bold)) },
            weekdays = weekdays.map { measurer.measure(it.take(1), header) },
            wk = measurer.measure("Wk", header),
            weeks = (0..53).map { measurer.measure("$it", style) },
        )
    }
}

/** A month of the year, small: as `MonthGrid` draws a month, but on one canvas (the year's twelve
 *  as composables, some three thousand of them, were slow to show on a small phone), its holidays
 *  coloured only, several countries' underlined. A tap on a holiday shows its holidays. */
@Composable
private fun SmallMonth(
    calendar: Calendar,
    m: CalendarMonth,
    dark: Boolean,
    glyphs: Glyphs,
    selected: Pair<Int, Int>?,
    select: (Int, Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colours = MaterialTheme.colorScheme
    val dim = colours.onSurfaceVariant.copy(alpha = 0.7f)
    val band = with(LocalDensity.current) { glyphs.wk.size.height.toDp() } + 4.dp
    val cell = 20.dp
    // What a screen reader says: the month and its holidays (its days are not apart, as on a month's grid).
    val said = remember(calendar, m) {
        val names = calendar.holidays.filter { it.month == m.month }.map { "${it.day}, ${it.name}, ${calendar.countries.getOrNull(it.country)?.second.orEmpty()}" }
        (listOf(m.title) + names).joinToString("; ")
    }
    Column(modifier) {
        Text(m.title, style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 4.dp))
        Canvas(
            Modifier.fillMaxWidth().height(band + cell * m.weeks.size)
                .semantics { contentDescription = said }
                .pointerInput(calendar, m) {
                    detectTapGestures { at ->
                        // The week numbers' column, then the seven days', as MonthGrid's weights (0.8 and 1).
                        val unit = size.width / 7.8f
                        val (x, y) = (at.x - 0.8f * unit) / unit to (at.y - band.toPx()) / cell.toPx()
                        val day = if (x < 0 || y < 0) 0 else m.weeks.getOrNull(y.toInt())?.days?.getOrNull(x.toInt()) ?: 0
                        if (day > 0 && calendar.holidaysOn(m.month, day).isNotEmpty()) select(m.month, day)
                    }
                },
        ) {
            val unit = size.width / 7.8f
            val top = band.toPx()
            val row = cell.toPx()
            val x = { k: Int -> 0.8f * unit + k * unit }
            val centred = { t: TextLayoutResult, cx: Float, cy: Float -> Offset(cx - t.size.width / 2f, cy - t.size.height / 2f) }
            // The weekdays on the theme's grey, the top corners round; the weekend's columns go on down.
            val r = CornerRadius(6.dp.toPx())
            drawPath(Path().apply { addRoundRect(RoundRect(Rect(0f, 0f, size.width, top), topLeft = r, topRight = r)) }, colours.surfaceVariant)
            calendar.weekend.forEach { k -> drawRect(colours.surfaceVariant, Offset(x(k), top), Size(unit, row * m.weeks.size)) }
            drawText(glyphs.wk, dim, centred(glyphs.wk, 0.4f * unit, top / 2))
            glyphs.weekdays.forEachIndexed { k, t -> drawText(t, if (k in calendar.weekend) dim else colours.onSurfaceVariant, centred(t, x(k) + unit / 2, top / 2)) }
            val circle = 9.dp.toPx()
            val ring = 2.dp.toPx()
            m.weeks.forEachIndexed { i, w ->
                val cy = top + i * row + row / 2
                glyphs.weeks.getOrNull(w.number)?.let { drawText(it, dim, centred(it, 0.4f * unit, cy)) }
                w.days.forEachIndexed { k, day ->
                    if (day <= 0 || day > 31) return@forEachIndexed
                    val cx = x(k) + unit / 2
                    val of = calendar.holidayOf(m.month, day)
                    val today = calendar.isToday(m, day)
                    if (today) drawCircle(colours.primary, circle, Offset(cx, cy))
                    // The day tapped: ringed in its country's colour.
                    if (selected == (m.month to day)) drawCircle(countryColour(of.firstOrNull() ?: 0, dark), circle - ring / 2, Offset(cx, cy), style = Stroke(ring))
                    val colour = when {
                        today -> colours.onPrimary
                        of.isNotEmpty() -> countryColour(of.first(), dark)
                        k in calendar.weekend -> colours.onSurfaceVariant
                        else -> colours.onSurface
                    }
                    val t = (if (of.isNotEmpty() || today) glyphs.bold else glyphs.plain)[day]
                    // Several countries' holiday: underlined (no room for the dots).
                    drawText(t, colour, centred(t, cx, cy), textDecoration = if (of.size > 1) TextDecoration.Underline else null)
                }
            }
        }
    }
}

@Composable
private fun Dot(colour: Color, size: Dp) {
    Box(Modifier.size(size).clip(CircleShape).background(colour))
}
