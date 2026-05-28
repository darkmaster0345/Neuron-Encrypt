package com.neuronencrypt.app.ui.theme

import android.app.Activity
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.Typography
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import com.neuronencrypt.app.R

// Colors mirrored from neuron-encrypt/src/gui.rs Palette struct
val Background = Color(0xFF080808)
val Surface = Color(0xFF101010)
val SurfaceHi = Color(0xFF1A1A1A)
val Border = Color(0xFF282828)
val Accent = Color(0xFF6366F1)
val AccentHover = Color(0xFF818CF8)
val TextHi = Color(0xFFF5F5F5)
val TextMed = Color(0xFF9A9A9A)
val TextLo = Color(0xFF4A4A4A)
val Success = Color(0xFF10B981)
val Error = Color(0xFFF43F5E)
val Warning = Color(0xFFF59E0B)

private val NeuronDarkColorScheme = darkColorScheme(
    primary = Accent,
    onPrimary = Color.White,
    secondary = AccentHover,
    onSecondary = Color.White,
    background = Background,
    onBackground = TextHi,
    surface = Surface,
    onSurface = TextHi,
    surfaceVariant = SurfaceHi,
    onSurfaceVariant = TextMed,
    error = Error,
    onError = Color.White,
    outline = Border,
    outlineVariant = Border,
)

val JetBrainsMonoFamily = FontFamily(
    Font(R.font.jetbrains_mono_regular, FontWeight.Normal)
)

val NeuronTypography = Typography(
    headlineLarge = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.Bold,
        fontSize = 24.sp,
        color = TextHi
    ),
    headlineMedium = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.Bold,
        fontSize = 20.sp,
        color = TextHi
    ),
    titleLarge = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 18.sp,
        color = TextHi
    ),
    titleMedium = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.Medium,
        fontSize = 16.sp,
        color = TextHi
    ),
    bodyLarge = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        color = TextMed
    ),
    bodyMedium = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.Normal,
        fontSize = 13.sp,
        color = TextMed
    ),
    bodySmall = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.Normal,
        fontSize = 11.sp,
        color = TextLo
    ),
    labelLarge = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 14.sp,
        color = TextHi
    ),
    labelMedium = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.Medium,
        fontSize = 12.sp,
        color = TextMed
    ),
    labelSmall = TextStyle(
        fontFamily = JetBrainsMonoFamily,
        fontWeight = FontWeight.Medium,
        fontSize = 10.sp,
        color = TextLo
    ),
)

@Composable
fun NeuronEncryptTheme(content: @Composable () -> Unit) {
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            window.statusBarColor = Background.toArgb()
            window.navigationBarColor = Background.toArgb()
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = false
        }
    }

    MaterialTheme(
        colorScheme = NeuronDarkColorScheme,
        typography = NeuronTypography,
        content = content
    )
}
