package com.neuronencrypt.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.neuronencrypt.app.ui.theme.*

@Composable
fun HomeScreen(
    onEncryptDecrypt: () -> Unit,
    onBatch: () -> Unit,
    onAbout: () -> Unit
) {
    var showAbout by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Background)
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        // Badge
        Surface(
            color = Accent.copy(alpha = 0.12f),
            shape = RoundedCornerShape(11.dp),
            modifier = Modifier.border(1.dp, Border, RoundedCornerShape(11.dp))
        ) {
            Text(
                text = "LOCAL ONLY",
                style = MaterialTheme.typography.labelSmall,
                color = AccentHover,
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp)
            )
        }

        Spacer(modifier = Modifier.height(24.dp))

        Text(
            text = "NEURON ENCRYPT",
            style = MaterialTheme.typography.headlineLarge,
            color = TextHi
        )

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = "Local file encryption.\nNo accounts. No internet.",
            style = MaterialTheme.typography.bodyLarge,
            color = TextMed,
            textAlign = TextAlign.Center
        )

        Spacer(modifier = Modifier.height(48.dp))

        // Primary action button
        Button(
            onClick = onEncryptDecrypt,
            modifier = Modifier
                .fillMaxWidth()
                .height(56.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = Accent
            ),
            shape = RoundedCornerShape(12.dp)
        ) {
            Text(
                text = "Encrypt or Decrypt a file",
                style = MaterialTheme.typography.labelLarge,
                color = TextHi
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        // Batch button
        OutlinedButton(
            onClick = onBatch,
            modifier = Modifier
                .fillMaxWidth()
                .height(56.dp),
            colors = ButtonDefaults.outlinedButtonColors(
                contentColor = TextHi
            ),
            border = ButtonDefaults.outlinedButtonBorder.copy(
                brush = androidx.compose.ui.graphics.SolidColor(Border)
            ),
            shape = RoundedCornerShape(12.dp)
        ) {
            Text(
                text = "Batch upload",
                style = MaterialTheme.typography.labelLarge,
                color = TextMed
            )
        }

        Spacer(modifier = Modifier.height(32.dp))

        TextButton(onClick = {
            onAbout()
            showAbout = true
        }) {
            Text(
                text = "About",
                style = MaterialTheme.typography.bodySmall,
                color = TextLo
            )
        }
    }

    if (showAbout) {
        AlertDialog(
            onDismissRequest = { showAbout = false },
            containerColor = Surface,
            title = {
                Text("Neuron Encrypt", color = TextHi)
            },
            text = {
                Column {
                    Text("Version 2.0.0", color = TextMed, fontSize = 13.sp)
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        "AES-256-GCM-SIV + Argon2id + HKDF-SHA512",
                        color = TextMed,
                        fontSize = 12.sp
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        "GPLv3 -- Copyright (c) 2024-2026 Ubaid ur Rehman",
                        color = TextLo,
                        fontSize = 11.sp
                    )
                }
            },
            confirmButton = {
                TextButton(onClick = { showAbout = false }) {
                    Text("Close", color = Accent)
                }
            }
        )
    }
}
