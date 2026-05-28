package com.neuronencrypt.app

enum class Strength { None, Weak, Fair, Strong, Elite }

data class StrengthResult(
    val strength: Strength,
    val fraction: Float,
    val label: String
)

fun evalStrength(password: String): StrengthResult {
    if (password.isEmpty()) {
        return StrengthResult(Strength.None, 0.0f, "None")
    }

    var score = 0.0f
    var hasUpper = false
    var hasDigit = false
    var hasSymbol = false
    val len = password.length

    for (c in password) {
        if (c.isUpperCase() && c.code in 0x41..0x5A) hasUpper = true
        if (c.isDigit() && c.code in 0x30..0x39) hasDigit = true
        if (!c.isLetterOrDigit()) hasSymbol = true
    }

    if (len >= 8) score += 1.0f
    if (len >= 12) score += 1.0f
    if (len >= 16) score += 1.0f
    if (hasUpper) score += 1.0f
    if (hasDigit) score += 1.0f
    if (hasSymbol) score += 1.0f

    score = score.coerceIn(0.0f, 6.0f)

    return when {
        score < 2.0f -> StrengthResult(Strength.Weak, score / 6.0f, "Weak")
        score < 3.5f -> StrengthResult(Strength.Fair, score / 6.0f, "Fair")
        score < 5.0f -> StrengthResult(Strength.Strong, score / 6.0f, "Strong")
        else -> StrengthResult(Strength.Elite, score / 6.0f, "Elite")
    }
}
