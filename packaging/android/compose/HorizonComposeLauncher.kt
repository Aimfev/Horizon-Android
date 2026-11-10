package dev.horizon.android.compose

import android.os.Build
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.NavigationRailItemDefaults
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Snackbar
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Google Sans Typography & Jetpack Compose Material You (Material 3 Expressive 2026)
 * Android Launcher UI for Horizon Android (`crates/horizon-theme/fonts/GoogleSans-*.ttf`).
 */
val GoogleSansFontFamily = FontFamily.SansSerif

val HorizonGoogleSansTypography = Typography(
    displayLarge = TextStyle(
        fontFamily = GoogleSansFontFamily,
        fontWeight = FontWeight.Bold,
        fontSize = 36.sp,
        lineHeight = 44.sp,
        letterSpacing = (-0.5).sp,
    ),
    headlineMedium = TextStyle(
        fontFamily = GoogleSansFontFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 22.sp,
        lineHeight = 28.sp,
    ),
    titleLarge = TextStyle(
        fontFamily = GoogleSansFontFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 18.sp,
        lineHeight = 24.sp,
    ),
    titleMedium = TextStyle(
        fontFamily = GoogleSansFontFamily,
        fontWeight = FontWeight.Medium,
        fontSize = 15.sp,
        lineHeight = 20.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = GoogleSansFontFamily,
        fontWeight = FontWeight.Normal,
        fontSize = 13.sp,
        lineHeight = 18.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = GoogleSansFontFamily,
        fontWeight = FontWeight.Medium,
        fontSize = 13.sp,
        lineHeight = 18.sp,
    ),
)

private val EmeraldForestDarkScheme = darkColorScheme(
    primary = Color(0xFF34D399),
    onPrimary = Color(0xFF06291D),
    primaryContainer = Color(0xFF124A36),
    onPrimaryContainer = Color(0xFFA7F3D0),
    secondary = Color(0xFF6EE7B7),
    onSecondary = Color(0xFF0A3325),
    secondaryContainer = Color(0xFF1B3B30),
    onSecondaryContainer = Color(0xFFD1FAE5),
    tertiary = Color(0xFF38BDF8),
    background = Color(0xFF0D1714),
    surface = Color(0xFF152620),
    surfaceVariant = Color(0xFF1C332B),
    onSurface = Color(0xFFF5F2FC),
    onSurfaceVariant = Color(0xFFA3B8B0),
    outline = Color(0xFF2D4A40),
)

enum class HorizonDestination(val label: String) {
    Play("Play"),
    Mods("Mods"),
    Packs("Resource Pack"),
    Shaders("Shaders"),
    Compiler("Compiler"),
    Installed("Installed"),
    Settings("Settings"),
    Skins("Skins"),
}

enum class HorizonCompilerMode(val label: String) {
    JarMod("Java .jar -> Horizon"),
    Modpack("Modpack (.mrpack)"),
    DataPackUpgrade("26.3 Data Pack"),
    NativeSoBuild("Native .so Build"),
}

@Composable
fun HorizonMaterialYouTheme(
    darkTheme: Boolean = true,
    dynamicColor: Boolean = true,
    content: @Composable () -> Unit,
) {
    val context = LocalContext.current
    val colorScheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }
        darkTheme -> EmeraldForestDarkScheme
        else -> lightColorScheme()
    }
    MaterialTheme(
        colorScheme = colorScheme,
        typography = HorizonGoogleSansTypography,
        content = content,
    )
}

@Composable
fun AnimatedMaterialYouAuroraBackground(modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    val transition = rememberInfiniteTransition(label = "material_you_aurora")
    val phaseA by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 7200, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "phaseA",
    )
    val phaseB by transition.animateFloat(
        initialValue = 1f,
        targetValue = 0f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 9600, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "phaseB",
    )

    Canvas(modifier = modifier.fillMaxSize().background(scheme.background)) {
        drawCircle(
            brush = Brush.radialGradient(
                colors = listOf(scheme.primary.copy(alpha = 0.16f), Color.Transparent),
                center = Offset(size.width * (0.75f - 0.18f * phaseA), size.height * (0.22f + 0.12f * phaseB)),
                radius = size.minDimension * 0.65f,
            ),
        )
        drawCircle(
            brush = Brush.radialGradient(
                colors = listOf(scheme.secondaryContainer.copy(alpha = 0.28f), Color.Transparent),
                center = Offset(size.width * (0.24f + 0.16f * phaseB), size.height * (0.78f - 0.14f * phaseA)),
                radius = size.minDimension * 0.60f,
            ),
        )
    }
}

@Composable
fun HorizonComposeLauncherApp(
    playerName: String = "byzindian",
    profileNumber: Int = 1,
    activeModsCount: Int = 12,
    onLaunchGame: () -> Unit = {},
    onOpenControlEditor: () -> Unit = {},
    onRunPortPackCompiler: (HorizonCompilerMode, String) -> Unit = { _, _ -> },
) {
    HorizonMaterialYouTheme {
        var currentPage by remember { mutableStateOf(HorizonDestination.Play) }
        var compilerMode by remember { mutableStateOf(HorizonCompilerMode.JarMod) }
        var compilerQuery by remember { mutableStateOf("") }

        Box(modifier = Modifier.fillMaxSize()) {
            AnimatedMaterialYouAuroraBackground()

            Row(modifier = Modifier.fillMaxSize().padding(12.dp)) {
                // Left Jetpack Compose Material You NavigationRail with Stadium Pill Indicators
                NavigationRail(
                    modifier = Modifier
                        .fillMaxHeight()
                        .width(172.dp)
                        .clip(RoundedCornerShape(18.dp)),
                    containerColor = MaterialTheme.colorScheme.surface.copy(alpha = 0.92f),
                ) {
                    HorizonDestination.entries.forEach { dest ->
                        val selected = currentPage == dest
                        NavigationRailItem(
                            selected = selected,
                            onClick = { currentPage = dest },
                            icon = {
                                Box(
                                    modifier = Modifier
                                        .size(10.dp)
                                        .clip(CircleShape)
                                        .background(
                                            if (selected) MaterialTheme.colorScheme.primary
                                            else MaterialTheme.colorScheme.onSurfaceVariant
                                        )
                                )
                            },
                            label = {
                                Text(
                                    text = dest.label,
                                    style = MaterialTheme.typography.labelLarge,
                                )
                            },
                            colors = NavigationRailItemDefaults.colors(
                                indicatorColor = MaterialTheme.colorScheme.secondaryContainer,
                            ),
                        )
                    }
                }

                Spacer(modifier = Modifier.width(16.dp))

                // Main Content Area with Spring AnimatedContent
                AnimatedContent(
                    targetState = currentPage,
                    transitionSpec = {
                        fadeIn(animationSpec = spring(stiffness = Spring.StiffnessMediumLow)) togetherWith
                            fadeOut(animationSpec = tween(140))
                    },
                    modifier = Modifier.weight(1f).fillMaxHeight(),
                    label = "page_content",
                ) { page ->
                    when (page) {
                        HorizonDestination.Play -> HorizonPlayDashboard(
                            profileNumber = profileNumber,
                            activeModsCount = activeModsCount,
                            onLaunchGame = onLaunchGame,
                            onOpenCompiler = { currentPage = HorizonDestination.Compiler },
                            onOpenMods = { currentPage = HorizonDestination.Mods },
                        )
                        HorizonDestination.Compiler -> HorizonCompilerWorkbench(
                            mode = compilerMode,
                            onModeChange = { compilerMode = it },
                            query = compilerQuery,
                            onQueryChange = { compilerQuery = it },
                            onExecute = { onRunPortPackCompiler(compilerMode, compilerQuery) },
                        )
                        else -> Surface(
                            modifier = Modifier.fillMaxSize(),
                            shape = RoundedCornerShape(18.dp),
                            color = MaterialTheme.colorScheme.surface,
                        ) {
                            Box(contentAlignment = Alignment.Center) {
                                Text(
                                    text = "${page.label} — Jetpack Compose Material You",
                                    style = MaterialTheme.typography.headlineMedium,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun HorizonPlayDashboard(
    profileNumber: Int,
    activeModsCount: Int,
    onLaunchGame: () -> Unit,
    onOpenCompiler: () -> Unit,
    onOpenMods: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // 1. Hero Banner ElevatedCard with integrated Play Split Button + FilterChip Row
        ElevatedCard(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(18.dp),
            colors = CardDefaults.elevatedCardColors(
                containerColor = MaterialTheme.colorScheme.surface,
            ),
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column {
                        Text(
                            text = "Ready to play",
                            style = MaterialTheme.typography.headlineMedium,
                        )
                        Text(
                            text = "Minecraft 26.3 • Profile $profileNumber • Vulkan / compatibility",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        Button(
                            onClick = onLaunchGame,
                            shape = RoundedCornerShape(topStart = 12.dp, bottomStart = 12.dp, topEnd = 4.dp, bottomEnd = 4.dp),
                        ) {
                            Text("Play Minecraft 26.3")
                        }
                        FilledTonalButton(
                            onClick = {},
                            shape = RoundedCornerShape(topStart = 4.dp, bottomStart = 4.dp, topEnd = 12.dp, bottomEnd = 12.dp),
                        ) {
                            Text("Profile $profileNumber ▾")
                        }
                    }
                }
                Spacer(modifier = Modifier.height(10.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(
                        selected = true,
                        onClick = {},
                        label = { Text("Renderer: Vulkan / compatibility") },
                    )
                    FilterChip(
                        selected = true,
                        onClick = {},
                        label = { Text("FPS: Max Uncapped (260)") },
                    )
                    AssistChip(
                        onClick = {},
                        label = { Text("Controls: GoyControl (Default Custom)") },
                    )
                }
            }
        }

        // 2. 2-Column Material You Bento Grid (Left: What's New + Single Discord/GitHub; Right: Mod Loader & Compiler)
        Row(
            modifier = Modifier.weight(1f).fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Card(
                modifier = Modifier.weight(0.58f).fillMaxHeight(),
                shape = RoundedCornerShape(16.dp),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.72f),
                ),
            ) {
                Column(
                    modifier = Modifier.fillMaxSize().padding(16.dp),
                    verticalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(
                            text = "CHANGELOG — MINECRAFT 26.3 • HORIZON ANDROID v1.0.1",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        Text("• Uncapped Max FPS (260 FPS) with Vulkan non-blocking swapchain & 120Hz+ display")
                        Text("• Native horizon-port-pack compiler (.jar -> .hmod + .hnmod) & Modrinth auto-port")
                        Text("• Google Sans typography, Jetpack Compose Material You UI & GoyControl snap-grid")
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilledTonalButton(onClick = {}) { Text("Discord") }
                        FilledTonalButton(onClick = {}) { Text("GitHub") }
                    }
                }
            }

            Card(
                modifier = Modifier.weight(0.42f).fillMaxHeight(),
                shape = RoundedCornerShape(16.dp),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.72f),
                ),
            ) {
                Column(
                    modifier = Modifier.fillMaxSize().padding(16.dp),
                    verticalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(
                            text = "Mod Loader & Compiler",
                            style = MaterialTheme.typography.titleMedium,
                        )
                        Text(
                            text = "horizon-port-pack • API 415 • .hnmod/.hmod",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        Text("• Active Profile Mods: $activeModsCount enabled")
                        Text("• 60 Built-in Horizon Mods & Shaders Ready")
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = onOpenCompiler) { Text("Open Compiler") }
                        FilledTonalButton(onClick = onOpenMods) { Text("Browse Mods") }
                    }
                }
            }
        }
    }
}

@Composable
private fun HorizonCompilerWorkbench(
    mode: HorizonCompilerMode,
    onModeChange: (HorizonCompilerMode) -> Unit,
    query: String,
    onQueryChange: (String) -> Unit,
    onExecute: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "horizon-port-pack & Native Compiler",
                style = MaterialTheme.typography.titleLarge,
            )
            SingleChoiceSegmentedButtonRow {
                HorizonCompilerMode.entries.forEachIndexed { idx, item ->
                    SegmentedButton(
                        selected = mode == item,
                        onClick = { onModeChange(item) },
                        shape = SegmentedButtonDefaults.itemShape(
                            index = idx,
                            count = HorizonCompilerMode.entries.size,
                        ),
                    ) {
                        Text(item.label)
                    }
                }
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = query,
                onValueChange = onQueryChange,
                modifier = Modifier.weight(1f),
                singleLine = true,
                placeholder = { Text("Enter Modrinth slug/URL or local .jar / .mrpack path...") },
            )
            Button(onClick = onExecute) {
                Text(
                    if (mode == HorizonCompilerMode.NativeSoBuild) "Build Native .so"
                    else "Port & Compile"
                )
            }
        }
    }
}
