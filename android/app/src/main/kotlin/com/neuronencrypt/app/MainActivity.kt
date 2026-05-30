package com.neuronencrypt.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.*
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.neuronencrypt.app.ui.BatchScreen
import com.neuronencrypt.app.ui.HomeScreen
import com.neuronencrypt.app.ui.SingleFileScreen
import com.neuronencrypt.app.ui.theme.NeuronEncryptTheme

class MainActivity : ComponentActivity() {

    private var sharedUriState = mutableStateOf<Uri?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Parse intent on cold start
        sharedUriState.value = handleIntent(intent)

        setContent {
            NeuronEncryptTheme {
                val navController = rememberNavController()

                // Route automatically if we have a shared Uri
                LaunchedEffect(sharedUriState.value) {
                    val uri = sharedUriState.value
                    if (uri != null) {
                        // Encode URI to pass safely as navigation argument
                        val encodedUri = Uri.encode(uri.toString())
                        navController.navigate("single?uri=$encodedUri") {
                            popUpTo("home") { saveState = true }
                            launchSingleTop = true
                        }
                        // Reset intent uri state after routing
                        sharedUriState.value = null
                    }
                }

                NavHost(navController = navController, startDestination = "home") {
                    composable("home") {
                        HomeScreen(
                            onEncryptDecrypt = { navController.navigate("single") },
                            onBatch = { navController.navigate("batch") }
                        )
                    }

                    composable(
                        route = "single?uri={uri}",
                        arguments = listOf(
                            navArgument("uri") {
                                type = NavType.StringType
                                nullable = true
                                defaultValue = null
                            }
                        )
                    ) { backStackEntry ->
                        val uriStr = backStackEntry.arguments?.getString("uri")
                        val uri = uriStr?.let { Uri.parse(Uri.decode(it)) }
                        SingleFileScreen(
                            onBack = { navController.popBackStack() },
                            sharedUri = uri
                        )
                    }

                    composable("batch") {
                        BatchScreen(
                            onBack = { navController.popBackStack() }
                        )
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        setIntent(intent)
        sharedUriState.value = handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?): Uri? {
        if (intent == null) return null
        return when (intent.action) {
            Intent.ACTION_VIEW -> intent.data
            Intent.ACTION_SEND -> {
                val streamUri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                streamUri ?: intent.data
            }
            else -> null
        }
    }
}
