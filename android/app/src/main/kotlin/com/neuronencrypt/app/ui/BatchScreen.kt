package com.neuronencrypt.app.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
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

data class BatchItem(
    val uri: Uri,
    val name: String,
    val size: Long,
    var status: String = "Pending",
    var progress: Float = 0f,
    var error: String? = null,
    var isWiped: Boolean = false
)

sealed class BatchStatus {
    object Idle : BatchStatus()
    data class Processing(
        val currentIndex: Int,
        val total: Int,
        val currentFileName: String,
        val currentStage: String,
        val progress: Float
    ) : BatchStatus()
    data class Done(val items: List<BatchItem>) : BatchStatus()
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BatchScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    var selectedFiles by remember { mutableStateOf<List<BatchItem>>(emptyList()) }
    var fileMode by remember { mutableStateOf(FileMode.Encrypt) }

    var password by remember { mutableStateOf("") }
    var confirmPassword by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }

    var batchStatus by remember { mutableStateOf<BatchStatus>(BatchStatus.Idle) }
    val cancelFlag = remember { AtomicBoolean(false) }

    val filePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenMultipleDocuments()
    ) { uris ->
        if (uris.isNotEmpty()) {
            val list = uris.map { uri ->
                persistUriPermission(context, uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                val info = getFileInfo(context, uri)
                BatchItem(uri = uri, name = info.first, size = info.second)
            }
            selectedFiles = list

            // Auto-detect mode based on files
            val vx2Count = list.count { it.name.endsWith(".vx2", ignoreCase = true) }
            fileMode = if (vx2Count > list.size / 2) FileMode.Decrypt else FileMode.Encrypt
            batchStatus = BatchStatus.Idle
        }
    }

    val dirPickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree()
    ) { treeUri ->
        if (treeUri != null && selectedFiles.isNotEmpty()) {
            persistUriPermission(
                context,
                treeUri,
                Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            )
            cancelFlag.set(false)

            scope.launch {
                val items = selectedFiles.map { it.copy() }
                val total = items.size

                val parentDocId = DocumentsContract.getTreeDocumentId(treeUri)
                val parentDocUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, parentDocId)

                for (i in 0 until total) {
                    if (cancelFlag.get()) {
                        items[i].status = "Cancelled"
                        continue
                    }

                    val item = items[i]
                    item.status = "Processing"
                    batchStatus = BatchStatus.Processing(i, total, item.name, "Initializing...", 0f)

                    val defaultName = defaultOutputName(fileMode, item.name)

                    var outUri: Uri? = null
                    try {
                        outUri = DocumentsContract.createDocument(
                            context.contentResolver,
                            parentDocUri,
                            "*/*",
                            defaultName
                        )

                        if (outUri == null) {
                            item.status = "Failed"
                            item.error = "Could not create destination file"
                            continue
                        }

                        val passwordBytes = password.toByteArray()
                        val listener = object : ProgressListener {
                            override fun onProgress(fraction: Float, stage: String) {
                                scope.launch {
                                    item.progress = fraction
                                    batchStatus = BatchStatus.Processing(i, total, item.name, stage, fraction)
                                }
                            }
                        }

                        val code = if (fileMode == FileMode.Encrypt) {
                            encrypt(context.contentResolver, item.uri, outUri, passwordBytes, listener, cancelFlag)
                        } else {
                            decrypt(context.contentResolver, item.uri, outUri, passwordBytes, listener, cancelFlag)
                        }

                        if (code == 0) {
                            item.status = "Success"
                        } else {
                            deleteDocumentQuietly(context, outUri)
                            item.status = if (cancelFlag.get()) "Cancelled" else "Failed"
                            item.error = if (cancelFlag.get()) "Cancelled" else "Native error code: $code"
                        }
                    } catch (e: Exception) {
                        outUri?.let { deleteDocumentQuietly(context, it) }
                        item.status = if (cancelFlag.get()) "Cancelled" else "Failed"
                        item.error = if (cancelFlag.get()) "Cancelled" else e.message ?: "Unknown error"
                    }
                }

                // Clear passwords
                password = ""
                confirmPassword = ""
                cancelFlag.set(false)
                batchStatus = BatchStatus.Done(items)
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("BATCH UPLOAD", style = MaterialTheme.typography.titleLarge) },
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
                .padding(24.dp)
        ) {
            if (batchStatus is BatchStatus.Idle) {
                // Pick button
                Button(
                    onClick = { filePickerLauncher.launch(arrayOf("*/*")) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(56.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Surface),
                    shape = RoundedCornerShape(12.dp),
                    border = ButtonDefaults.outlinedButtonBorder.copy(
                        brush = androidx.compose.ui.graphics.SolidColor(Border)
                    )
                ) {
                    Icon(Icons.Default.Add, contentDescription = "Add Files", tint = Accent)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Select Files", style = MaterialTheme.typography.labelLarge, color = TextHi)
                }

                if (selectedFiles.isNotEmpty()) {
                    Spacer(modifier = Modifier.height(16.dp))

                    Text("${selectedFiles.size} FILES SELECTED", style = MaterialTheme.typography.labelSmall, color = TextLo)
                    Spacer(modifier = Modifier.height(8.dp))

                    LazyColumn(
                        modifier = Modifier
                            .weight(1f)
                            .border(1.dp, Border, RoundedCornerShape(8.dp))
                            .background(Surface)
                            .padding(8.dp)
                    ) {
                        items(selectedFiles) { file ->
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(vertical = 6.dp, horizontal = 8.dp),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = file.name,
                                    color = TextHi,
                                    style = MaterialTheme.typography.bodyMedium,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.weight(1f)
                                )
                                Spacer(modifier = Modifier.width(16.dp))
                                Text(
                                    text = formatSize(file.size),
                                    color = TextLo,
                                    style = MaterialTheme.typography.bodySmall
                                )
                            }
                        }
                    }

                    Spacer(modifier = Modifier.height(16.dp))

                    // Mode tabs
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

                    Spacer(modifier = Modifier.height(16.dp))

                    // Passphrase inputs
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
                        Spacer(modifier = Modifier.height(12.dp))
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

                    Spacer(modifier = Modifier.height(24.dp))

                    val isEnabled = password.length >= 8 &&
                            (fileMode == FileMode.Decrypt || (password == confirmPassword))

                    Button(
                        onClick = { dirPickerLauncher.launch(null) },
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
                            text = "CHOOSE DESTINATION & PROCESS",
                            style = MaterialTheme.typography.labelLarge,
                            color = if (isEnabled) TextHi else TextLo
                        )
                    }
                } else {
                    Box(
                        modifier = Modifier.weight(1f),
                        contentAlignment = Alignment.Center
                    ) {
                        Text("No files selected", color = TextLo, style = MaterialTheme.typography.bodyLarge)
                    }
                }
            }

            if (batchStatus is BatchStatus.Processing) {
                val proc = batchStatus as BatchStatus.Processing
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    CircularProgressIndicator(
                        progress = proc.progress,
                        color = Accent,
                        trackColor = Border,
                        modifier = Modifier.size(80.dp)
                    )
                    Spacer(modifier = Modifier.height(24.dp))
                    Text(
                        text = "Processing file ${proc.currentIndex + 1} of ${proc.total}",
                        style = MaterialTheme.typography.titleLarge,
                        color = TextHi
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = proc.currentFileName,
                        style = MaterialTheme.typography.bodyLarge,
                        color = TextMed,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text(
                        text = proc.currentStage,
                        style = MaterialTheme.typography.bodyMedium,
                        color = TextLo
                    )
                    Spacer(modifier = Modifier.height(48.dp))
                    OutlinedButton(
                        onClick = { cancelFlag.set(true) },
                        border = ButtonDefaults.outlinedButtonBorder.copy(
                            brush = androidx.compose.ui.graphics.SolidColor(Border)
                        ),
                        shape = RoundedCornerShape(8.dp)
                    ) {
                        Text("CANCEL BATCH", color = Error, fontFamily = JetBrainsMonoFamily)
                    }
                }
            }

            if (batchStatus is BatchStatus.Done) {
                val done = batchStatus as BatchStatus.Done
                val successes = done.items.count { it.status == "Success" }

                Column(
                    modifier = Modifier.fillMaxSize()
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(
                            imageVector = if (successes == done.items.size) Icons.Default.CheckCircle else Icons.Default.Warning,
                            contentDescription = "Done",
                            tint = if (successes == done.items.size) Success else Warning,
                            modifier = Modifier.size(36.dp)
                        )
                        Spacer(modifier = Modifier.width(12.dp))
                        Text(
                            text = "$successes of ${done.items.size} processed",
                            style = MaterialTheme.typography.headlineMedium,
                            color = TextHi
                        )
                    }

                    Spacer(modifier = Modifier.height(16.dp))

                    LazyColumn(
                        modifier = Modifier
                            .weight(1f)
                            .border(1.dp, Border, RoundedCornerShape(8.dp))
                            .background(Surface)
                            .padding(8.dp)
                    ) {
                        items(done.items) { item ->
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(vertical = 8.dp, horizontal = 12.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Icon(
                                    imageVector = when (item.status) {
                                        "Success" -> Icons.Default.Check
                                        "Failed" -> Icons.Default.Close
                                        else -> Icons.Default.Help
                                    },
                                    contentDescription = item.status,
                                    tint = when (item.status) {
                                        "Success" -> Success
                                        "Failed" -> Error
                                        else -> TextLo
                                    },
                                    modifier = Modifier.size(18.dp)
                                )
                                Spacer(modifier = Modifier.width(12.dp))
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        text = item.name,
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = TextHi,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                    if (item.error != null) {
                                        Text(
                                            text = item.error!!,
                                            style = MaterialTheme.typography.bodySmall,
                                            color = Error
                                        )
                                    }
                                }
                            }
                        }
                    }

                    Spacer(modifier = Modifier.height(24.dp))

                    // Wipe successful files
                    var anyWiped by remember { mutableStateOf(false) }
                    val successItems = done.items.filter { it.status == "Success" && !it.isWiped }

                    if (successItems.isNotEmpty() && !anyWiped) {
                        Button(
                            onClick = {
                                scope.launch {
                                    for (item in successItems) {
                                        try {
                                            val deleted = DocumentsContract.deleteDocument(
                                                context.contentResolver,
                                                item.uri
                                            )
                                            if (deleted) {
                                                item.isWiped = true
                                            } else {
                                                context.contentResolver.delete(item.uri, null, null)
                                                item.isWiped = true
                                            }
                                        } catch (e: Exception) {
                                            // ignore
                                        }
                                    }
                                    anyWiped = true
                                }
                            },
                            colors = ButtonDefaults.buttonColors(containerColor = Error),
                            modifier = Modifier.fillMaxWidth(),
                            shape = RoundedCornerShape(8.dp)
                        ) {
                            Text("WIPE SUCCESSFUL SOURCE FILES", color = TextHi, fontFamily = JetBrainsMonoFamily)
                        }
                        Spacer(modifier = Modifier.height(12.dp))
                    } else if (anyWiped) {
                        Surface(
                            color = Success.copy(alpha = 0.08f),
                            shape = RoundedCornerShape(8.dp),
                            modifier = Modifier
                                .fillMaxWidth()
                                .border(1.dp, Success, RoundedCornerShape(8.dp))
                        ) {
                            Text(
                                "Source files wiped successfully.",
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
                            selectedFiles = emptyList()
                            batchStatus = BatchStatus.Idle
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
