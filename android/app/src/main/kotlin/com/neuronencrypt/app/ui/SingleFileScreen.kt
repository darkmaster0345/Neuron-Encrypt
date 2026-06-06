package com.neuronencrypt.app.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.material3.TabRowDefaults.tabIndicatorOffset
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.neuronencrypt.app.*
import com.neuronencrypt.app.ui.theme.*
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.roundToInt

enum class FileMode { Encrypt, Decrypt }

sealed class ProcessStatus {
    object Idle : ProcessStatus()
    data class Running(val progress: Float, val stage: String) : ProcessStatus()
    data class Success(
        val inputName: String,
        val outputName: String,
        val durationMs: Long,
        val sizeBytes: Long,
        val isWiped: Boolean = false
    ) : ProcessStatus()
    data class Error(val message: String) : ProcessStatus()
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SingleFileScreen(
    onBack: () -> Unit,
    sharedUri: Uri? = null // For incoming SEND/VIEW intents
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val scrollState = rememberScrollState()

    var inputUri by remember { mutableStateOf<Uri?>(null) }
    var inputName by remember { mutableStateOf("") }
    var inputSize by remember { mutableStateOf(0L) }
    var detectedMagic by remember { mutableStateOf<String?>(null) }

    var fileMode by remember { mutableStateOf(FileMode.Encrypt) }

    var password by remember { mutableStateOf("") }
    var confirmPassword by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }

    var status by remember { mutableStateOf<ProcessStatus>(ProcessStatus.Idle) }
    val cancelFlag = remember { AtomicBoolean(false) }

    // Helpers to query info and magic
    fun selectFile(uri: Uri) {
        persistUriPermission(context, uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        inputUri = uri
        val info = getFileInfo(context, uri)
        inputName = info.first
        inputSize = info.second
        val magic = detectMagicBytes(context, uri)
        detectedMagic = magic
        if (magic == "VAULTX02" || magic == "VAULTX03") {
            fileMode = FileMode.Decrypt
        } else {
            fileMode = FileMode.Encrypt
        }
        status = ProcessStatus.Idle
    }

    // Handle sharedUri on initial load
    LaunchedEffect(sharedUri) {
        if (sharedUri != null) {
            selectFile(sharedUri)
        }
    }

    val filePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri ->
        if (uri != null) {
            selectFile(uri)
        }
    }

    val savePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.CreateDocument("*/*")
    ) { uri ->
        if (uri != null && inputUri != null) {
            cancelFlag.set(false)
            status = ProcessStatus.Running(0.0f, "Initializing...")

            scope.launch {
                val startTime = System.currentTimeMillis()
                val passwordBytes = password.toByteArray()
                val listener = object : ProgressListener {
                    override fun onProgress(fraction: Float, stage: String) {
                        scope.launch {
                            status = ProcessStatus.Running(fraction, stage)
                        }
                    }
                }

                try {
                    val code = if (fileMode == FileMode.Encrypt) {
                        encrypt(context.contentResolver, inputUri!!, uri, passwordBytes, listener, cancelFlag)
                    } else {
                        decrypt(context.contentResolver, inputUri!!, uri, passwordBytes, listener, cancelFlag)
                    }

                    if (code == 0) {
                        val duration = System.currentTimeMillis() - startTime
                        val outInfo = getFileInfo(context, uri)
                        status = ProcessStatus.Success(
                            inputName = inputName,
                            outputName = outInfo.first,
                            durationMs = duration,
                            sizeBytes = inputSize
                        )
                        // Clear fields
                        password = ""
                        confirmPassword = ""
                    } else {
                        deleteDocumentQuietly(context, uri)
                        status = ProcessStatus.Error("JNI execution returned error code: $code")
                    }
                } catch (e: Exception) {
                    deleteDocumentQuietly(context, uri)
                    status = ProcessStatus.Error(e.message ?: "Operation failed with unexpected exception")
                } finally {
                    password = ""
                    confirmPassword = ""
                    cancelFlag.set(false)
                }
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("SINGLE FILE", style = MaterialTheme.typography.titleLarge) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back", tint = TextHi)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Background,
                    titleContentColor = TextHi
                )
            )
        },
        containerColor = Background
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .background(Background)
                .verticalScroll(scrollState)
                .padding(24.dp)
        ) {
            if (status is ProcessStatus.Idle || status is ProcessStatus.Error) {
                // File Picker trigger box
                Card(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { filePickerLauncher.launch(arrayOf("*/*")) }
                        .border(1.dp, Border, RoundedCornerShape(12.dp)),
                    colors = CardDefaults.cardColors(containerColor = Surface),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        Icon(
                            imageVector = Icons.Default.UploadFile,
                            contentDescription = "Select File",
                            tint = Accent,
                            modifier = Modifier.size(48.dp)
                        )
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = if (inputUri == null) "Select a file to encrypt/decrypt" else inputName,
                            style = MaterialTheme.typography.titleMedium,
                            color = TextHi,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                            textAlign = TextAlign.Center
                        )
                        if (inputUri != null) {
                            Spacer(modifier = Modifier.height(8.dp))
                            Text(
                                text = formatSize(inputSize),
                                style = MaterialTheme.typography.bodyMedium,
                                color = TextMed
                            )
                        }
                    }
                }

                if (inputUri != null) {
                    Spacer(modifier = Modifier.height(24.dp))

                    // Mode Selection Tab (with warning/info if magic detected)
                    TabRow(
                        selectedTabIndex = if (fileMode == FileMode.Encrypt) 0 else 1,
                        containerColor = Surface,
                        contentColor = Accent,
                        indicator = { tabPositions ->
                            TabRowDefaults.SecondaryIndicator(
                                Modifier.tabIndicatorOffset(tabPositions[if (fileMode == FileMode.Encrypt) 0 else 1]),
                                color = Accent
                            )
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .border(1.dp, Border, RoundedCornerShape(8.dp))
                    ) {
                        Tab(
                            selected = fileMode == FileMode.Encrypt,
                            onClick = { fileMode = FileMode.Encrypt },
                            text = { Text("Encrypt", fontFamily = JetBrainsMonoFamily) }
                        )
                        Tab(
                            selected = fileMode == FileMode.Decrypt,
                            onClick = { fileMode = FileMode.Decrypt },
                            text = { Text("Decrypt", fontFamily = JetBrainsMonoFamily) }
                        )
                    }

                    // Format detection indicator
                    if (detectedMagic != null) {
                        Spacer(modifier = Modifier.height(12.dp))
                        Surface(
                            color = Accent.copy(alpha = 0.08f),
                            shape = RoundedCornerShape(8.dp),
                            modifier = Modifier.border(1.dp, Border, RoundedCornerShape(8.dp))
                        ) {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(12.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Icon(Icons.Default.Info, contentDescription = "Info", tint = Accent, modifier = Modifier.size(18.dp))
                                Spacer(modifier = Modifier.width(8.dp))
                                Text(
                                    text = "Auto-detected $detectedMagic archive format.",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = TextMed
                                )
                            }
                        }
                    }

                    Spacer(modifier = Modifier.height(24.dp))

                    // Password Inputs
                    Text("PASSPHRASE", style = MaterialTheme.typography.labelSmall, color = TextLo)
                    Spacer(modifier = Modifier.height(8.dp))

                    OutlinedTextField(
                        value = password,
                        onValueChange = { password = it },
                        modifier = Modifier.fillMaxWidth(),
                        visualTransformation = if (showPassword) VisualTransformation.None else PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                        trailingIcon = {
                            IconButton(onClick = { showPassword = !showPassword }) {
                                Icon(
                                    imageVector = if (showPassword) Icons.Default.VisibilityOff else Icons.Default.Visibility,
                                    contentDescription = if (showPassword) "Hide password" else "Show password",
                                    tint = TextMed
                                )
                            }
                        },
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedContainerColor = Surface,
                            unfocusedContainerColor = Surface,
                            focusedBorderColor = Accent,
                            unfocusedBorderColor = Border
                        ),
                        singleLine = true
                    )

                    if (password.isNotEmpty()) {
                        val strengthResult = evalStrength(password)
                        val strengthColor = when (strengthResult.strength) {
                            Strength.None -> TextLo
                            Strength.Weak -> Error
                            Strength.Fair -> Warning
                            Strength.Strong -> Accent
                            Strength.Elite -> Success
                        }

                        Spacer(modifier = Modifier.height(8.dp))
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            LinearProgressIndicator(
                                progress = strengthResult.fraction,
                                color = strengthColor,
                                trackColor = Border,
                                modifier = Modifier
                                    .weight(1f)
                                    .height(4.dp)
                            )
                            Spacer(modifier = Modifier.width(12.dp))
                            Text(
                                text = strengthResult.label,
                                style = MaterialTheme.typography.labelSmall,
                                color = strengthColor,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }

                    if (fileMode == FileMode.Encrypt) {
                        Spacer(modifier = Modifier.height(16.dp))
                        Text("CONFIRM PASSPHRASE", style = MaterialTheme.typography.labelSmall, color = TextLo)
                        Spacer(modifier = Modifier.height(8.dp))

                        OutlinedTextField(
                            value = confirmPassword,
                            onValueChange = { confirmPassword = it },
                            modifier = Modifier.fillMaxWidth(),
                            visualTransformation = if (showPassword) VisualTransformation.None else PasswordVisualTransformation(),
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                            colors = OutlinedTextFieldDefaults.colors(
                                focusedContainerColor = Surface,
                                unfocusedContainerColor = Surface,
                                focusedBorderColor = Accent,
                                unfocusedBorderColor = Border
                            ),
                            singleLine = true
                        )

                        if (password.isNotEmpty() && confirmPassword.isNotEmpty() && password != confirmPassword) {
                            Spacer(modifier = Modifier.height(6.dp))
                            Text("Passphrases do not match", color = Error, style = MaterialTheme.typography.bodySmall)
                        }
                    }

                    Spacer(modifier = Modifier.height(32.dp))

                    // Execute Button
                    val isEnabled = password.length >= 8 &&
                            (fileMode == FileMode.Decrypt || (password == confirmPassword))

                    Button(
                        onClick = {
                            val defaultName = defaultOutputName(fileMode, inputName)
                            savePickerLauncher.launch(defaultName)
                        },
                        enabled = isEnabled,
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(50.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = Accent,
                            disabledContainerColor = SurfaceHi
                        ),
                        shape = RoundedCornerShape(8.dp)
                    ) {
                        Text(
                            text = if (fileMode == FileMode.Encrypt) "ENCRYPT" else "DECRYPT",
                            style = MaterialTheme.typography.labelLarge,
                            color = if (isEnabled) TextHi else TextLo
                        )
                    }
                }

                // Error display
                if (status is ProcessStatus.Error) {
                    Spacer(modifier = Modifier.height(24.dp))
                    Surface(
                        color = Error.copy(alpha = 0.08f),
                        shape = RoundedCornerShape(8.dp),
                        modifier = Modifier.border(1.dp, Error, RoundedCornerShape(8.dp))
                    ) {
                        Column(modifier = Modifier.padding(16.dp)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Icon(Icons.Default.Error, contentDescription = "Error", tint = Error)
                                Spacer(modifier = Modifier.width(8.dp))
                                Text("Operation failed", color = Error, style = MaterialTheme.typography.titleMedium)
                            }
                            Spacer(modifier = Modifier.height(8.dp))
                            Text((status as ProcessStatus.Error).message, color = TextHi, style = MaterialTheme.typography.bodyMedium)
                        }
                    }
                }
            }

            // Running state
            if (status is ProcessStatus.Running) {
                val running = status as ProcessStatus.Running
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 48.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    CircularProgressIndicator(
                        progress = running.progress,
                        color = Accent,
                        trackColor = Border,
                        modifier = Modifier.size(80.dp)
                    )
                    Spacer(modifier = Modifier.height(24.dp))
                    Text(
                        text = "${(running.progress * 100).roundToInt()}%",
                        style = MaterialTheme.typography.headlineLarge,
                        color = TextHi
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                    Text(
                        text = running.stage,
                        style = MaterialTheme.typography.bodyLarge,
                        color = TextMed,
                        textAlign = TextAlign.Center
                    )
                    Spacer(modifier = Modifier.height(48.dp))
                    OutlinedButton(
                        onClick = { cancelFlag.set(true) },
                        border = ButtonDefaults.outlinedButtonBorder.copy(
                            brush = androidx.compose.ui.graphics.SolidColor(Border)
                        ),
                        shape = RoundedCornerShape(8.dp)
                    ) {
                        Text("CANCEL", color = Error, fontFamily = JetBrainsMonoFamily)
                    }
                }
            }

            // Success state
            if (status is ProcessStatus.Success) {
                val success = status as ProcessStatus.Success
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 12.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Icon(
                        imageVector = Icons.Default.CheckCircle,
                        contentDescription = "Success",
                        tint = Success,
                        modifier = Modifier.size(72.dp)
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text("Complete", style = MaterialTheme.typography.headlineMedium, color = TextHi)
                    Spacer(modifier = Modifier.height(24.dp))

                    Card(
                        modifier = Modifier
                            .fillMaxWidth()
                            .border(1.dp, Border, RoundedCornerShape(8.dp)),
                        colors = CardDefaults.cardColors(containerColor = Surface),
                        shape = RoundedCornerShape(8.dp)
                    ) {
                        Column(modifier = Modifier.padding(16.dp)) {
                            DetailRow(label = "Source", value = success.inputName)
                            DetailRow(label = "Output", value = success.outputName)
                            DetailRow(label = "Size", value = formatSize(success.sizeBytes))

                            val seconds = success.durationMs / 1000f
                            val speed = if (seconds > 0) formatSize((success.sizeBytes / seconds).toLong()) + "/s" else "N/A"
                            DetailRow(label = "Time elapsed", value = String.format("%.2fs", seconds))
                            DetailRow(label = "Average speed", value = speed)
                        }
                    }

                    Spacer(modifier = Modifier.height(32.dp))

                    if (!success.isWiped) {
                        Button(
                            onClick = {
                                try {
                                    val deleted = android.provider.DocumentsContract.deleteDocument(
                                        context.contentResolver,
                                        inputUri!!
                                    )
                                    if (deleted) {
                                        status = success.copy(isWiped = true)
                                    } else {
                                        // Try standard content resolver delete
                                        val rows = context.contentResolver.delete(inputUri!!, null, null)
                                        if (rows > 0) {
                                            status = success.copy(isWiped = true)
                                        } else {
                                            scope.launch {
                                                status = ProcessStatus.Error("Storage provider denied deletion request.")
                                            }
                                        }
                                    }
                                } catch (e: Exception) {
                                    status = ProcessStatus.Error("Delete failed: ${e.message}")
                                }
                            },
                            colors = ButtonDefaults.buttonColors(containerColor = Error),
                            modifier = Modifier.fillMaxWidth(),
                            shape = RoundedCornerShape(8.dp)
                        ) {
                            Text("WIPE SOURCE FILE", color = TextHi, fontFamily = JetBrainsMonoFamily)
                        }
                        Spacer(modifier = Modifier.height(12.dp))
                    } else {
                        Surface(
                            color = Success.copy(alpha = 0.08f),
                            shape = RoundedCornerShape(8.dp),
                            modifier = Modifier
                                .fillMaxWidth()
                                .border(1.dp, Success, RoundedCornerShape(8.dp))
                        ) {
                            Text(
                                "Source file wiped successfully.",
                                color = Success,
                                modifier = Modifier.padding(16.dp),
                                textAlign = TextAlign.Center,
                                style = MaterialTheme.typography.bodyMedium
                            )
                        }
                        Spacer(modifier = Modifier.height(16.dp))
                    }

                    OutlinedButton(
                        onClick = {
                            inputUri = null
                            inputName = ""
                            inputSize = 0
                            detectedMagic = null
                            status = ProcessStatus.Idle
                        },
                        modifier = Modifier.fillMaxWidth(),
                        border = ButtonDefaults.outlinedButtonBorder.copy(
                            brush = androidx.compose.ui.graphics.SolidColor(Border)
                        ),
                        shape = RoundedCornerShape(8.dp)
                    ) {
                        Text("DONE", color = TextMed, fontFamily = JetBrainsMonoFamily)
                    }
                }
            }
        }
    }
}

@Composable
fun DetailRow(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = TextLo)
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            color = TextHi,
            textAlign = TextAlign.End,
            modifier = Modifier.padding(start = 16.dp),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis
        )
    }
}

fun formatSize(bytes: Long): String {
    if (bytes < 1024) return "$bytes B"
    val exp = (Math.log(bytes.toDouble()) / Math.log(1024.0)).toInt()
    val pre = "KMGTPE"[exp - 1].toString()
    return String.format("%.2f %sB", bytes / Math.pow(1024.0, exp.toDouble()), pre)
}

fun defaultOutputName(mode: FileMode, inputName: String): String {
    val name = inputName.ifBlank { "file" }
    return when (mode) {
        FileMode.Encrypt -> "$name.vx2"
        FileMode.Decrypt -> {
            if (name.endsWith(".vx2", ignoreCase = true)) {
                val stripped = name.dropLast(4)
                stripped.ifEmpty { "decrypted" }
            } else {
                "$name.decrypted"
            }
        }
    }
}

fun persistUriPermission(context: Context, uri: Uri, flags: Int) {
    try {
        context.contentResolver.takePersistableUriPermission(uri, flags)
    } catch (_: SecurityException) {
        // Some providers grant only transient access.
    } catch (_: IllegalArgumentException) {
        // Ignore non-SAF or non-persistable URIs.
    }
}

fun deleteDocumentQuietly(context: Context, uri: Uri) {
    try {
        if (!android.provider.DocumentsContract.deleteDocument(context.contentResolver, uri)) {
            context.contentResolver.delete(uri, null, null)
        }
    } catch (_: Exception) {
        try {
            context.contentResolver.delete(uri, null, null)
        } catch (_: Exception) {
            // Best-effort cleanup only.
        }
    }
}

fun detectMagicBytes(context: Context, uri: Uri): String? {
    try {
        context.contentResolver.openInputStream(uri)?.use { stream ->
            val magic = ByteArray(8)
            val n = stream.read(magic)
            if (n == 8) {
                if (magic.contentEquals("VAULTX02".toByteArray())) return "VAULTX02"
                if (magic.contentEquals("VAULTX03".toByteArray())) return "VAULTX03"
            }
        }
    } catch (e: Exception) {
        // Ignore
    }
    return null
}

fun getFileInfo(context: Context, uri: Uri): Pair<String, Long> {
    var name = "unknown"
    var size = 0L
    try {
        context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
            val nameIdx = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
            val sizeIdx = cursor.getColumnIndex(android.provider.OpenableColumns.SIZE)
            if (cursor.moveToFirst()) {
                if (nameIdx != -1) name = cursor.getString(nameIdx)
                if (sizeIdx != -1) size = cursor.getLong(sizeIdx)
            }
        }
    } catch (e: Exception) {
        // Ignore
    }
    return Pair(name, size)
}
