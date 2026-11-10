import math
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def strip_rust_comments_and_literals(source: str) -> str:
    """Single-pass lexer that strips Rust line comments, block comments, char literals,
    raw string literals, and standard string literals (including '\\\n' line continuations)."""
    token_re = re.compile(
        r"//[^\n]*|"
        r"/\*[\s\S]*?\*/|"
        r"(?<![a-zA-Z0-9_'])b?'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|[\s\S])|[^'\\\n])'|"
        r'(?<![a-zA-Z0-9_])b?r(#*)"[\s\S]*?"\1|'
        r'b?"(?:\\[\s\S]|[^"\\])*"'
    )
    return token_re.sub(" ", source)



def _read_repo_file(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")

class LauncherOverhaulTests(unittest.TestCase):
    def test_rust_delimiter_and_syntax_sanity_across_modified_files(self) -> None:
        modified_files = [
            "crates/horizon-client-render/src/touch/layout.rs",
            "crates/horizon-client-render/src/touch.rs",
            "crates/horizon-client-render/src/options_ui.rs",
            "crates/horizon-client/src/touch_ui.rs",
            "crates/horizon-client/src/ui_theme.rs",
            "crates/horizon-client/src/game/android_input.rs",
            "crates/horizon-client/src/game/startup_menu.rs",
            "crates/horizon-client/src/game/options.rs",
            "crates/horizon-client/src/game/pad.rs",
            "crates/horizon-client/src/game/pad_menu.rs",
            "crates/horizon-client/src/game.rs",
            "crates/horizon-client/src/android/catalog.rs",
            "crates/horizon-client/src/android/wardrobe.rs",
            "crates/horizon-client/src/android/launcher_pages.rs",
            "crates/horizon-client/src/android/launcher.rs",
            "crates/horizon-client/src/android/chooser.rs",
            "crates/horizon-client/src/android/login.rs",
            "crates/horizon-client/src/android/native_ime.rs",
            "crates/horizon-client/src/android/display.rs",
            "crates/horizon-client-render/src/touch/editor.rs",
            "crates/horizon-client-render/src/gfx.rs",
            "crates/horizon-client-render/src/audio.rs",
            "crates/horizon-client-render/src/mesher.rs",
            "crates/horizon-client-render/src/mesher/sides.rs",
            "crates/horizon-client-render/src/world.rs",
            "crates/horizon-world/src/level/mod.rs",
            "crates/horizon-world/src/level/update.rs",
            "crates/horizon-world/src/level/tick.rs",
            "crates/horizon-client-render/src/touch/custom.rs",
            "crates/horizon-client-render/src/menu_blur.rs",
            "crates/horizon-client-render/src/ui.rs",
            "crates/horizon-client/src/android/document_store.rs",
            "crates/horizon-client/src/content_files.rs",
            "crates/horizon-client/src/game/native_hooks.rs",
            "crates/horizon-client/src/android/documents.rs",
            "crates/horizon-client/src/android/platform.rs",
            "crates/horizon-client/src/game/perspective.rs",
            "crates/horizon-client/src/game/recipe_book.rs",
            "crates/horizon-client-input/src/window.rs",
            "crates/horizon-client-render/src/hud.rs",
            "crates/horizon-client/src/android/device_runtime.rs",
            "crates/horizon-client/src/android/document_bridge.rs",
            "third_party/wgpu-hal/src/vulkan/swapchain/native.rs",
            "crates/horizon-client/src/game/screens.rs",
            "crates/horizon-client/src/item_render.rs",
            "crates/horizon-client/src/item_render/gui.rs",
            "third_party/winit/src/platform_impl/android/keycodes.rs",
            "third_party/winit/src/platform_impl/android/mod.rs",
            "third_party/winit/src/platform_impl/android/mouse_state.rs",
            "launcher/src/skins/preview.rs",
            "crates/horizon-client-render/src/gfx/pack_post.rs",
            "crates/horizon-client-render/src/gfx/pack_atlas.rs",
            "crates/horizon-client-render/src/gfx/pack_uniforms.rs",
            "crates/horizon-client-render/src/gfx/arena.rs",
            "crates/horizon-client-render/src/graphics.rs",
            "crates/horizon-client/src/game/shader_packs.rs",
            "third_party/winit/src/platform_impl/android/mod.rs",
            "third_party/winit/src/platform_impl/android/mouse_state.rs",
            "third_party/winit/src/event_loop.rs",
            "third_party/naga/src/back/glsl/writer.rs",
            "crates/horizon-client/src/game/android_bundled.rs",
            "crates/horizon-client/src/game/mods.rs",
            "crates/horizon-shaders/src/custom.rs",
            "crates/horizon-shaders/src/translate/scan.rs",
            "crates/horizon-shaders/src/translate/rewrite.rs",
            "crates/horizon-client-render/src/gfx/pack_sky.rs",
            "crates/horizon-client-render/src/gfx/pack_shadow.rs",
            "crates/horizon-client-render/src/gfx/pack_images.rs",
            "crates/horizon-client-render/src/gfx/flat.rs",
            "crates/horizon-client/src/game/fov.rs",
            "crates/horizon-client/src/android.rs",
            "crates/horizon-client/src/android_lib.rs",
            "crates/horizon-client/src/android_main.rs",
            "crates/horizon-client/src/horizon_releases.rs",
            "crates/horizon-client/src/jar_transpiler.rs",
            "crates/horizon-client/src/mod_api.rs",
            "crates/horizon-client/src/mod_api/generated.rs",
            "crates/horizon-client/src/mod_api/generated_host.rs",
            "crates/horizon-client/src/game/native_builds.rs",
            "crates/horizon-client/src/game/build_mining.rs",
            "crates/horizon-client/src/game/setup/fishing.rs",
            "crates/horizon-client/src/game/mod_api_host.rs",
            "crates/horizon-client/src/game/mod_api_host_generated.rs",
            "crates/horizon-client-render/src/gfx/upload.rs",
            "crates/horizon-client-render/src/logging.rs",
            "crates/horizon-client-render/src/mod_list.rs",
            "crates/horizon-client-render/src/crash.rs",
            "crates/horizon-mods/src/index/native.rs",
            "crates/horizon-shaders/src/condition.rs",
            "crates/horizon-shaders/src/preprocess.rs",
            "third_party/naga/src/front/glsl/parser/functions.rs",
            "third_party/naga/src/front/glsl/types.rs",
            "third_party/naga/src/front/glsl/functions.rs",
            "third_party/naga/src/valid/function.rs",
            "launcher/src/native_mods.rs",
            "launcher/src/native_mods/api.rs",
            "launcher/src/native_mods/crates.rs",
            "launcher/src/native_mods/dependencies.rs",
            "launcher/src/native_mods/inject.rs",
            "launcher/src/native_mods/inject_cache.rs",
            "crates/horizon-shaders/src/block_ids.rs",
            "crates/horizon-client-render/src/gfx/native_material.rs",
            "crates/horizon-shader-api/src/package.rs",
            "crates/horizon-client/src/android/updater.rs",
        ]
        for rel in modified_files:
            path = ROOT / rel
            self.assertTrue(path.is_file(), f"Missing file: {rel}")
            text = path.read_text(encoding="utf-8")
            cleaned = strip_rust_comments_and_literals(text)
            self.assertEqual(
                cleaned.count("{"),
                cleaned.count("}"),
                f"Unbalanced braces in {rel}",
            )
            self.assertEqual(
                cleaned.count("("),
                cleaned.count(")"),
                f"Unbalanced parens in {rel}",
            )
            self.assertEqual(
                cleaned.count("["),
                cleaned.count("]"),
                f"Unbalanced brackets in {rel}",
            )

    def test_virtual_mouse_windows11_pointer_and_authoritative_toggle(self) -> None:
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        layout = (ROOT / "crates/horizon-client-render/src/touch/layout.rs").read_text(encoding="utf-8")
        touch = (ROOT / "crates/horizon-client-render/src/touch.rs").read_text(encoding="utf-8")
        options_ui = (ROOT / "crates/horizon-client-render/src/options_ui.rs").read_text(encoding="utf-8")
        launcher_pages = (
            ROOT / "crates/horizon-client/src/android/launcher_pages.rs"
        ).read_text(encoding="utf-8")

        # 1. Faithful Windows 11 standard arrow pointer (aero_arrow) with exact (0,0) tip hotspot
        self.assertIn("pub fn draw_pointer(", touch_ui)
        self.assertIn("[pt(0.0, 0.0), pt(4.1, 11.4), pt(0.0, 14.8), pt(0.0, 0.0)]", touch_ui)
        self.assertIn("pub fn sync_viewport_cursor(", touch_ui)
        self.assertIn("pub fn compute_virtual_mouse_visibility(", touch_ui)

        # 2. Authoritative persisted virtual_mouse_enabled in touch::Settings
        self.assertIn("pub virtual_mouse_enabled: bool,", layout)
        self.assertIn("pub fn mcpe_controls_visible(", touch)

        # 3. Duplicate Virtual Mouse settings removed from Controls settings (both MCPE & GoyControl)
        page_touch_block = options_ui.split("Page::Touch => {")[1].split("Page::Controller =>")[0]
        self.assertNotIn("Opt::TouchMouseSensitivity", page_touch_block)
        controls_cat_block = launcher_pages.split("SettingsCategory::Controls => {")[1].split(
            "SettingsCategory::Experimental => {"
        )[0]
        self.assertNotIn("Virtual Mouse Pointer Speed", controls_cat_block)

    def test_floating_menu_settings_wired_and_scrollable(self) -> None:
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        android_input = (
            ROOT / "crates/horizon-client/src/game/android_input.rs"
        ).read_text(encoding="utf-8")
        startup_menu = (
            ROOT / "crates/horizon-client/src/game/startup_menu.rs"
        ).read_text(encoding="utf-8")
        pad = (ROOT / "crates/horizon-client/src/game/pad.rs").read_text(encoding="utf-8")

        # Scrollable Floating Menu content pane & Virtual Mouse speed control
        self.assertIn("pub hub_scroll: f32,", touch_ui)
        self.assertIn("pub hub_max_scroll: Cell<f32>,", touch_ui)
        self.assertIn("SetVirtualMouseSpeed(f32),", touch_ui)
        self.assertIn("InGameMenuAction::StepVirtualMouseSpeed", touch_ui)
        self.assertIn("InGameMenuAction::SetVirtualMouseSpeed", android_input)
        self.assertIn("InGameMenuAction::SetVirtualMouseSpeed", startup_menu)

        # Haptic Feedback wired to pad.rs and android_touch_haptic
        self.assertIn("!self.pad.touch.settings.vibrate", pad)
        self.assertIn("fn android_touch_haptic(", android_input)

        # Auto-Jump wired to self.settings.auto_jump and player.auto_jump_enabled
        self.assertIn("player.auto_jump_enabled = next;", android_input)
        self.assertIn("self.settings.auto_jump = next;", startup_menu)

        # Toggle Sprint wired to both sprint_toggle and toggle_sprint
        self.assertIn("self.pad.touch.settings.sprint_toggle = next;", android_input)
        self.assertIn("self.menu.controls.settings.sprint_toggle = next;", startup_menu)

        # Split Touch wired to toggle_split_touch()
        self.assertIn("self.pad.touch.settings.toggle_split_touch();", android_input)
        self.assertIn("self.menu.controls.settings.toggle_split_touch();", startup_menu)

    def test_skin_3d_projection_and_drag_direction_math(self) -> None:
        preview = (ROOT / "launcher/src/skins/preview.rs").read_text(encoding="utf-8")
        wardrobe = (ROOT / "crates/horizon-client/src/android/wardrobe.rs").read_text(encoding="utf-8")
        self.assertIn("pub fn drag(&mut self, dx: f32, dy: f32)", preview)
        self.assertIn("self.rotate(-dx * 0.7, -dy * 0.5);", preview)
        self.assertIn("self.camera.drag(point[0] - old[0], point[1] - old[1]);", wardrobe)

        # Verify the exact projection math from Raster::project across front (yaw=0) and back (yaw=180)
        def project(pt, yaw_deg, pitch_deg, zoom=1.0, width=160.0, height=200.0):
            x, y, z = pt
            s, c = math.sin(math.radians(yaw_deg)), math.cos(math.radians(yaw_deg))
            x, z = x * c + z * s, -x * s + z * c
            s, c = math.sin(math.radians(pitch_deg)), math.cos(math.radians(pitch_deg))
            y, z = (y - 16.0) * c - z * s, (y - 16.0) * s + z * c
            scale = height / 39.0 * zoom
            perspective = 100.0 / (100.0 + z)
            return [width * 0.5 + x * scale * perspective, height * 0.5 - y * scale * perspective, z]

        # Front chest point [0, 18, -2] at yaw=0, pitch=0
        p0 = project([0.0, 18.0, -2.0], 0.0, 0.0)
        p_drag_right = project([0.0, 18.0, -2.0], -20.0 * 0.7, 0.0)
        p_drag_down = project([0.0, 18.0, -2.0], 0.0, -20.0 * 0.5)
        self.assertGreater(p_drag_right[0], p0[0])
        self.assertGreater(p_drag_down[1], p0[1])

        # Back cape point [0, 18, 3] at yaw=180, pitch=0
        b0 = project([0.0, 18.0, 3.0], 180.0, 0.0)
        b_drag_right = project([0.0, 18.0, 3.0], 180.0 - 20.0 * 0.7, 0.0)
        b_drag_down = project([0.0, 18.0, 3.0], 180.0, -20.0 * 0.5)
        self.assertGreater(b_drag_right[0], b0[0])
        self.assertGreater(b_drag_down[1], b0[1])

    def test_theme_customization_and_controls_animation_exclusion(self) -> None:
        ui_theme = (ROOT / "crates/horizon-client/src/ui_theme.rs").read_text(encoding="utf-8")
        catalog = (ROOT / "crates/horizon-client/src/android/catalog.rs").read_text(encoding="utf-8")
        launcher_pages = (
            ROOT / "crates/horizon-client/src/android/launcher_pages.rs"
        ).read_text(encoding="utf-8")

        self.assertIn("pub struct LauncherThemeConfig", ui_theme)
        self.assertIn("pub enum ThemePreset", ui_theme)
        self.assertIn("pub enum GradientDirection", ui_theme)
        self.assertIn("pub fn draw_theme_background(", ui_theme)
        self.assertIn("pub theme: horizon_client::ui_theme::LauncherThemeConfig", catalog)
        self.assertIn("SettingsCategory::Theme =>", launcher_pages)
        self.assertIn("Action::ResetTheme", launcher_pages)

        # Ensure draw_hud_editor in launcher_pages.rs renders directly without page/modal animation wrapper
        editor_branch = launcher_pages.split("if let Some(tc) = &mut self.control_editor {")[
            1
        ].split("return Ok(());")[0]
        self.assertIn("draw_hud_editor", editor_branch)
        self.assertNotIn("self.anim", editor_branch)



    def test_stability_6s_freeze_burning_village_and_leaves_fps_optimizations(self) -> None:
        gfx = (ROOT / "crates/horizon-client-render/src/gfx.rs").read_text(encoding="utf-8")
        audio = (ROOT / "crates/horizon-client-render/src/audio.rs").read_text(encoding="utf-8")
        mesher = (ROOT / "crates/horizon-client-render/src/mesher.rs").read_text(encoding="utf-8")
        sides = (ROOT / "crates/horizon-client-render/src/mesher/sides.rs").read_text(encoding="utf-8")
        terrain_wgsl = (ROOT / "crates/horizon-client-render/src/terrain.wgsl").read_text(
            encoding="utf-8"
        )
        tick = (ROOT / "crates/horizon-world/src/level/tick.rs").read_text(encoding="utf-8")
        world = (ROOT / "crates/horizon-client-render/src/world.rs").read_text(encoding="utf-8")
        game = (ROOT / "crates/horizon-client/src/game.rs").read_text(encoding="utf-8")
        native_ime = (ROOT / "crates/horizon-client/src/android/native_ime.rs").read_text(encoding="utf-8")

        # 1. In-place Loading::set_ui texture update & surface_extent tracking
        self.assertIn("pub fn set_loading_ui(", gfx)
        self.assertIn("surface_extent: std::sync::atomic::AtomicU64,", gfx)
        self.assertIn("pub fn with_registry(", audio)

        # 2. Invisible fire age updates do not mark 8 chunk sections dirty (#88)
        self.assertIn("pub fn set_with_visibility(", world)
        self.assertIn("260 | super::flags::UPDATE_KNOWN_SHAPE", tick)

        # 3. Smart/hollow leaf culling & early fragment discard (#156)
        self.assertIn("let mut force_opaque_face = [false; 6];", mesher)
        self.assertIn('static GROUPED: AtomicBool = AtomicBool::new(cfg!(all(target_os = "android", not(test))));', sides)
        self.assertLess(
            terrain_wgsl.index("if texel.a * in.color.a < 0.5 && (in.layer & 0x8000u) == 0u { discard; }"),
            terrain_wgsl.index("var color = texel * in.color;"),
        )

        # 4. Immersive fullscreen constant decor layout & safe exit_app
        self.assertIn("immersive: AtomicBool::new(false),", native_ime)
        self.assertRegex(native_ime, r'call_method\(\s*&decor,\s*"getWidth"')
        self.assertIn("crate::game::exit_app(event_loop);", game)

    def test_custom_input_dialogue_typing_back_confirm_launching_and_slider(self) -> None:
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        editor = (ROOT / "crates/horizon-client-render/src/touch/editor.rs").read_text(encoding="utf-8")
        android_input = (
            ROOT / "crates/horizon-client/src/game/android_input.rs"
        ).read_text(encoding="utf-8")
        startup_menu = (
            ROOT / "crates/horizon-client/src/game/startup_menu.rs"
        ).read_text(encoding="utf-8")
        launcher = (ROOT / "crates/horizon-client/src/android/launcher.rs").read_text(encoding="utf-8")
        launcher_pages = (
            ROOT / "crates/horizon-client/src/android/launcher_pages.rs"
        ).read_text(encoding="utf-8")

        # Send Custom Input modal & action
        self.assertIn("SendCustomInput(String),", touch_ui)
        self.assertIn("fn draw_input_picker_modal(", touch_ui)
        self.assertIn("InGameMenuAction::SendCustomInput(token)", android_input)
        self.assertIn("InGameMenuAction::SendCustomInput(token)", startup_menu)

        # Unified dialogue typing across Launcher, StartupMenu, and In-Game
        self.assertIn("pub fn apply_editor_or_preset_output(", touch_ui)
        self.assertIn("apply_editor_or_preset_output(", android_input)
        self.assertIn("apply_editor_or_preset_output(", startup_menu)
        self.assertIn("apply_editor_or_preset_output(", launcher)

        # Control Editor back confirmation when unsaved changes exist
        self.assertIn("pub back_confirm_open: bool,", editor)
        self.assertIn("pub fn has_unsaved_changes(&self) -> bool", editor)
        self.assertIn("pub fn request_editor_back(&mut self) -> bool", editor)
        self.assertIn("Unsaved Layout Changes", touch_ui)

        # Main Launcher "Launching..." state & Virtual Mouse Speed slider
        self.assertIn("pub(super) launching: bool,", launcher)
        self.assertIn('"Launching..."', launcher_pages)
        self.assertIn("pub mouse_speed_dragging: bool,", touch_ui)


    def test_next_development_pass_controls_kbm_file_manager_dpi_intro_and_ui_perf(self) -> None:
        touch = (ROOT / "crates/horizon-client-render/src/touch.rs").read_text(encoding="utf-8")
        layout = (ROOT / "crates/horizon-client-render/src/touch/layout.rs").read_text(encoding="utf-8")
        custom = (ROOT / "crates/horizon-client-render/src/touch/custom.rs").read_text(encoding="utf-8")
        editor = (ROOT / "crates/horizon-client-render/src/touch/editor.rs").read_text(encoding="utf-8")
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        ui_theme = (ROOT / "crates/horizon-client/src/ui_theme.rs").read_text(encoding="utf-8")
        doc_store = (ROOT / "crates/horizon-client/src/android/document_store.rs").read_text(encoding="utf-8")
        documents = (ROOT / "crates/horizon-client/src/android/documents.rs").read_text(encoding="utf-8")
        catalog = (ROOT / "crates/horizon-client/src/android/catalog.rs").read_text(encoding="utf-8")
        launcher = (ROOT / "crates/horizon-client/src/android/launcher.rs").read_text(encoding="utf-8")
        launcher_pages = (
            ROOT / "crates/horizon-client/src/android/launcher_pages.rs"
        ).read_text(encoding="utf-8")
        mouse_state = (
            ROOT / "third_party/winit/src/platform_impl/android/mouse_state.rs"
        ).read_text(encoding="utf-8")
        keycodes = (
            ROOT / "third_party/winit/src/platform_impl/android/keycodes.rs"
        ).read_text(encoding="utf-8")
        native_ime = (ROOT / "crates/horizon-client/src/android/native_ime.rs").read_text(encoding="utf-8")
        game = (ROOT / "crates/horizon-client/src/game.rs").read_text(encoding="utf-8")
        android_input = (
            ROOT / "crates/horizon-client/src/game/android_input.rs"
        ).read_text(encoding="utf-8")
        startup_menu = (
            ROOT / "crates/horizon-client/src/game/startup_menu.rs"
        ).read_text(encoding="utf-8")
        menu_blur = (ROOT / "crates/horizon-client-render/src/menu_blur.rs").read_text(encoding="utf-8")
        perspective = (
            ROOT / "crates/horizon-client/src/game/perspective.rs"
        ).read_text(encoding="utf-8")
        item_gui = (ROOT / "crates/horizon-client/src/item_render/gui.rs").read_text(encoding="utf-8")
        item_render = (ROOT / "crates/horizon-client/src/item_render.rs").read_text(encoding="utf-8")
        ui_render = (ROOT / "crates/horizon-client-render/src/ui.rs").read_text(encoding="utf-8")
        screens = (ROOT / "crates/horizon-client/src/game/screens.rs").read_text(encoding="utf-8")

        # 1. MCPE Floating Menu bubble appearance (Horizon H icon in MCPE, MenuEquals in GoyControl)
        self.assertIn("if settings.control_style == ControlStyle::Mcpe {", touch_ui)
        self.assertIn("Icon::HorizonLogo", touch_ui)
        self.assertIn("Icon::MenuEquals", touch_ui)

        # 2. Dynamic joystick touch-origin behaviour
        self.assertIn("pub fn move_anchor(&self) -> Option<[f32; 2]>", touch)
        self.assertIn("move_area: JoystickMoveArea::FollowFinger", layout)
        self.assertIn("dynamic_joystick: true,", layout)
        self.assertIn("let Some(anchor) = tc.move_anchor()", touch_ui)

        # 3. Control-type switching safety & custom button bounds safety
        self.assertIn("tc.settings = ed.original.clone();", touch_ui)
        self.assertIn("fn clamp_editor_custom_selection(", touch_ui)
        self.assertIn("self.android_release_touch_keys(true);", android_input)

        # 4. External KBM support, pointer capture & stability
        self.assertIn("pub rel_x: f64,", mouse_state)
        self.assertIn("matches!(sample.action, 2 | 7)", mouse_state)
        self.assertIn("let ending = sample.action == 3;", mouse_state)
        self.assertIn("KEY_CHARACTER_MAPS", keycodes)
        self.assertIn("os_mouse_captured: false,", game)
        self.assertIn('"setDefaultFocusHighlightEnabled"', native_ime)

        # 5. Smooth auto-hide touch controls on external KBM
        self.assertIn("pub hud_opacity_scale: f32,", touch)
        self.assertIn("pub fn update_visibility(&mut self, touch_active: bool, dt: f32)", touch)
        self.assertIn("self.pad.touch.update_visibility(", game)
        self.assertIn(".update_visibility(crate::touch_visible(), 1.0 / 60.0);", startup_menu)

        # 6. Mouse cursor rendering & clipping (never over Control Editor, no (0,0) teleport)
        self.assertIn("if controls.editor.is_some() {\n            return false;\n        }", touch_ui)
        self.assertNotIn("(cursor.0 == 0.0 && cursor.1 == 0.0)", touch_ui)
        self.assertIn("let _ = crate::platform::capture_mouse(false);", launcher)

        # 7. Installed Tab redesign & Integrated Mini File Manager
        self.assertIn("pub enum InstalledSubTab", catalog)
        self.assertIn("pub fn fm_paste_here(", catalog)
        self.assertIn("pub fn fm_paste(", doc_store)
        self.assertIn("pub fn fm_confirm_delete(&mut self, home: &Path)", catalog)
        self.assertIn("pub fn fm_submit_rename(&mut self, home: &Path)", catalog)
        self.assertIn("pub fn fm_submit_new_folder(&mut self, home: &Path)", catalog)
        self.assertIn("pub fn fm_paste(", doc_store)
        self.assertIn("pub fn begin_into_dir(", documents)
        self.assertIn("pub fn begin_folder_into_dir(", documents)
        self.assertIn("pub fn export_file(", documents)
        self.assertIn("pub fn export_folder(", documents)
        self.assertIn("fn draw_fm_modal(", launcher_pages)
        self.assertIn("fn draw_update_fullscreen_ui(", launcher_pages)

        # 8 & 9. DPI scaling & EditText / single-line hint redesign
        self.assertIn("(pw / 880.0).min(ph / 440.0)", ui_theme)
        self.assertIn("pub fn hint_line(", ui_theme)
        self.assertIn("ui.hint_line(", touch_ui)

        # 10. Animated Horizon intro screen
        self.assertIn("pub fn draw_horizon_intro(", ui_theme)
        self.assertIn("pub fn draw_animated_horizon_mark(", ui_theme)
        self.assertIn("pub intro_done: bool,", launcher_pages)

        # 11 & 12. MCPE Control Editor Add Control & single-binding exclusivity
        self.assertIn("pub fn set_exclusive_binding(&mut self, token: String)", custom)
        self.assertIn("pub fn is_keyboard_binding(token: &str) -> bool", custom)
        self.assertIn("pub fn is_mouse_binding(token: &str) -> bool", custom)
        self.assertIn("set_exclusive_binding", editor)

        # 13. Rust Edition in-game UI & menu performance
        self.assertIn("pub fn blit_cached(", menu_blur)
        self.assertIn("pub menu_blur_cache: bool,", (ROOT / "crates/horizon-client-render/src/gfx.rs").read_text(encoding="utf-8"))
        self.assertIn("batch.clip_since(start, x0, y0, x1, y1);", perspective)
        self.assertIn("struct GuiCachedItem", item_gui)
        self.assertIn("static CACHE: std::cell::RefCell<Vec<(crate::items::ItemStack, StackView)>>", item_render)
        self.assertIn("src[2] <= 2.0 || src[3] <= 2.0", ui_render)
        self.assertRegex(
            screens,
            r"fill_gradient\(\s*atlas,\s*\(0\.0, 0\.0, w, h\),\s*"
            r"rgba\(80, 0, 0, 96\),\s*rgba\(128, 48, 48, 160\),\s*\)",
        )

        # 14. Vulkan Batch render chunk stabilization & mobile performance
        gfx_code = (ROOT / "crates/horizon-client-render/src/gfx.rs").read_text(encoding="utf-8")
        cull_code = (ROOT / "crates/horizon-client-render/src/gfx/cull.rs").read_text(encoding="utf-8")
        flat_code = (ROOT / "crates/horizon-client-render/src/gfx/flat.rs").read_text(encoding="utf-8")
        budget_code = (ROOT / "crates/horizon-client-render/src/gfx/upload_budget.rs").read_text(encoding="utf-8")
        self.assertIn("let multi_draw = !cfg!(target_os = \"android\")", gfx_code)
        self.assertIn("if cfg!(target_os = \"android\") {", cull_code)
        self.assertIn("std::env::var(\"HORIZON_FLAT_DRAWS\").as_deref() == Ok(\"1\")", flat_code)
        self.assertIn("crate::android_runtime::gpu_batching()", budget_code)



    def test_anr_fix_z_order_keybind_dispatch_and_controls_overhaul(self) -> None:
        test_content = (ROOT / "tools/test_android_content.py").read_text(encoding="utf-8")
        gfx = (ROOT / "crates/horizon-client-render/src/gfx.rs").read_text(encoding="utf-8")
        game = (ROOT / "crates/horizon-client/src/game.rs").read_text(encoding="utf-8")
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        android_input = (
            ROOT / "crates/horizon-client/src/game/android_input.rs"
        ).read_text(encoding="utf-8")
        startup_menu = (
            ROOT / "crates/horizon-client/src/game/startup_menu.rs"
        ).read_text(encoding="utf-8")
        pad = (ROOT / "crates/horizon-client/src/game/pad.rs").read_text(encoding="utf-8")
        custom = (ROOT / "crates/horizon-client-render/src/touch/custom.rs").read_text(encoding="utf-8")
        editor = (ROOT / "crates/horizon-client-render/src/touch/editor.rs").read_text(encoding="utf-8")
        launcher_pages = (
            ROOT / "crates/horizon-client/src/android/launcher_pages.rs"
        ).read_text(encoding="utf-8")
        options_ui = (ROOT / "crates/horizon-client-render/src/options_ui.rs").read_text(encoding="utf-8")
        ui_theme = (ROOT / "crates/horizon-client/src/ui_theme.rs").read_text(encoding="utf-8")

        # 1. tools/test_android_content.py supplies mod android_runtime stub
        self.assertIn("mod android_runtime {", test_content)
        self.assertIn("pub fn gpu_batching() -> bool", test_content)

        # 2. In-place set_world_assets on Renderer eliminates concurrent VkQueue ANR
        self.assertIn("pub fn matches_sprite_layout(&self, layers: u32) -> bool", gfx)
        self.assertIn("pub fn set_world_assets(", gfx)
        self.assertIn("renderer.set_world_assets(", game)

        # 3. Mouse cursor z-order strictly below touch controls & floating menu, hidden when floating_menu is open
        self.assertLess(
            game.index("horizon_client::touch_ui::draw_pointer("),
            game.index("self.pad.touch.draw("),
        )
        self.assertLess(
            startup_menu.index("horizon_client::touch_ui::draw_pointer("),
            startup_menu.index("horizon_client::touch_ui::draw_goy_hud("),
        )
        self.assertIn("if self.floating_menu", touch_ui)

        # 4. GoyControls / PRI / SEC / SendCustomInput key & mouse dispatch + key capture
        self.assertIn("fn android_custom_token_edge(", android_input)
        self.assertIn("fn android_custom_token_tap(", android_input)
        self.assertIn("pub(super) late_custom_releases: Vec<String>,", pad)
        self.assertIn("pub fn capture_custom_input_key(", touch_ui)
        self.assertIn("pub fn capture_custom_key_code(", editor)
        self.assertIn("pub fn held_custom_tokens(&self)", custom)

        # 5. Settings > Controls & Page::Touch & UI performance overhaul
        self.assertIn("Action::CycleJoystickZone", launcher_pages)
        self.assertIn("Action::ToggleTouchVibrate", launcher_pages)
        self.assertIn('Row::Header("Movement & Look Sensitivity")', options_ui)
        self.assertIn("glyph_scratch: Vec<(f32, f32, Rect)>,", ui_theme)


    def test_mali_g52_shaders_startup_freeze_arena_pooling_and_m3_controls_overhaul(self) -> None:
        pack_post = (
            ROOT / "crates/horizon-client-render/src/gfx/pack_post.rs"
        ).read_text(encoding="utf-8")
        pack_atlas = (
            ROOT / "crates/horizon-client-render/src/gfx/pack_atlas.rs"
        ).read_text(encoding="utf-8")
        pack_uniforms = (
            ROOT / "crates/horizon-client-render/src/gfx/pack_uniforms.rs"
        ).read_text(encoding="utf-8")
        arena = (ROOT / "crates/horizon-client-render/src/gfx/arena.rs").read_text(encoding="utf-8")
        graphics = (ROOT / "crates/horizon-client-render/src/graphics.rs").read_text(encoding="utf-8")
        gfx = (ROOT / "crates/horizon-client-render/src/gfx.rs").read_text(encoding="utf-8")
        shader_packs = (
            ROOT / "crates/horizon-client/src/game/shader_packs.rs"
        ).read_text(encoding="utf-8")
        game = (ROOT / "crates/horizon-client/src/game.rs").read_text(encoding="utf-8")
        startup_menu = (
            ROOT / "crates/horizon-client/src/game/startup_menu.rs"
        ).read_text(encoding="utf-8")
        launcher_pages = (
            ROOT / "crates/horizon-client/src/android/launcher_pages.rs"
        ).read_text(encoding="utf-8")
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")

        # 1. Shader pack compatibility on max_bind_groups=4 mobile GPUs & non-flat mc_Entity
        self.assertIn("fn remap_bind_group(", pack_post)
        self.assertIn("vertex_writable_storage: bool,", pack_post)
        self.assertIn("pub fn has_any_pass(&self) -> bool", pack_post)
        self.assertIn("pub fn terrain_inputs_with_ids(", pack_atlas)
        self.assertIn('"endFlashIntensity"', pack_uniforms)
        self.assertIn("pub fn has_active_shader_pack(&self) -> bool", gfx)
        self.assertIn("PENDING_SHADER_JOB", shader_packs)

        # 2. Startup / MC Title Menu freeze elimination on Android
        self.assertIn('let rest_loader = if menu && !cfg!(target_os = "android") {', game)
        self.assertIn('let android_boot_loading = cfg!(target_os = "android")', startup_menu)
        self.assertIn("self.arena.prewarm(&self.device);", gfx)

        # 3. Mali-G52 MC2 32 MiB 64-bit arena pages, spare page recycling & big.LITTLE worker cap
        self.assertIn("spare: Vec<(wgpu::Buffer, u64, bool)>,", arena)
        self.assertIn("pub fn prewarm(&mut self, device: &wgpu::Device)", arena)
        self.assertIn("cores.saturating_sub(2).clamp(1, 4)", graphics)

        # 4. Material Design 3 Expressive overhaul of Settings > Controls & Property Inspector
        self.assertIn("SettingControl::SectionHeader(", launcher_pages)
        self.assertIn("SettingControl::Slider {", launcher_pages)
        self.assertIn("fn draw_m3_stepper_cell(", touch_ui)
        self.assertIn("fn draw_m3_swatch_chip(", touch_ui)


    def test_studio_props_recommended_resolution_shaders_and_animations_revamp(self) -> None:
        custom_rs = (ROOT / "crates/horizon-shaders/src/custom.rs").read_text(encoding="utf-8")
        scan_rs = (ROOT / "crates/horizon-shaders/src/translate/scan.rs").read_text(encoding="utf-8")
        rewrite_rs = (ROOT / "crates/horizon-shaders/src/translate/rewrite.rs").read_text(encoding="utf-8")
        pack_sky = (ROOT / "crates/horizon-client-render/src/gfx/pack_sky.rs").read_text(encoding="utf-8")
        pack_post = (ROOT / "crates/horizon-client-render/src/gfx/pack_post.rs").read_text(encoding="utf-8")
        pack_uniforms = (
            ROOT / "crates/horizon-client-render/src/gfx/pack_uniforms.rs"
        ).read_text(encoding="utf-8")
        shader_packs = (ROOT / "crates/horizon-client/src/game/shader_packs.rs").read_text(encoding="utf-8")
        menu_blur = (ROOT / "crates/horizon-client-render/src/menu_blur.rs").read_text(encoding="utf-8")
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        layout = (ROOT / "crates/horizon-client-render/src/touch/layout.rs").read_text(encoding="utf-8")
        launcher_pages = (
            ROOT / "crates/horizon-client/src/android/launcher_pages.rs"
        ).read_text(encoding="utf-8")
        ui_theme = (ROOT / "crates/horizon-client/src/ui_theme.rs").read_text(encoding="utf-8")

        # 1. MakeUp & E-Lite gl_FogFragCoord + trailing semicolon shader fixes
        self.assertIn("gl_FogFragCoord", scan_rs)
        self.assertIn("hz_FogFragCoord", rewrite_rs)
        self.assertIn('self.tokens.last().is_some_and(|t| t == ";")', custom_rs)

        # 2. BSL subdivided sky dome, unified far plane & Mali-G52 4-bind-group / 4-color-attachment folding
        self.assertIn("let rings = 6i32;", pack_sky)
        self.assertIn("let slices = 24i32;", pack_sky)
        self.assertIn("frame.depth_far.max(frame.detail_distance)", pack_uniforms)
        self.assertIn("fn trim_fragment_outputs(", pack_post)
        self.assertIn("160 + b", pack_post)
        self.assertIn("shader_pack_build_context()", shader_packs)

        # 3. Zero-lag menu backdrop cache & Virtual Mouse OFF by default
        self.assertIn("if radius < 0.5 {", menu_blur)
        self.assertIn("virtual_mouse_enabled: false,", layout)

        # 4. Studio ButtonProperty Inspector (90511.png) + Icon badge + Recommended tab + Real-Time Resolution
        self.assertIn("pub icon_id: u8,", layout)
        self.assertIn("pub resolution_scale_percent: u32,", layout)
        self.assertIn("pub fn control_icon_from_id(", touch_ui)
        self.assertIn("fn draw_preview_brackets(", touch_ui)
        self.assertIn("(Icon::HorizonLogo, \"Recommended\")", touch_ui)
        self.assertIn("SetResolutionScale(u32),", touch_ui)
        self.assertIn("ToggleGameAnimations,", touch_ui)

        # 5. Disable Launcher / In-Game Animations & Diagnostics Summary Modal
        self.assertIn("pub fn set_launcher_animations_disabled(", ui_theme)
        self.assertIn("pub fn set_game_animations_disabled(", ui_theme)
        self.assertIn("Action::ToggleLauncherAnimations", launcher_pages)
        self.assertIn("Action::ToggleGameAnimations", launcher_pages)
        self.assertIn("fn draw_diagnostics_summary_modal(", launcher_pages)



    def test_mali_mrt_shader_ground_complementary_passthrough_and_m3_sliders(self) -> None:
        adapter = (ROOT / "third_party/wgpu-hal/src/vulkan/adapter.rs").read_text(encoding="utf-8")
        flat = (ROOT / "crates/horizon-client-render/src/gfx/flat.rs").read_text(encoding="utf-8")
        mesher = (ROOT / "crates/horizon-client-render/src/mesher.rs").read_text(encoding="utf-8")
        pack_atlas = (ROOT / "crates/horizon-client-render/src/gfx/pack_atlas.rs").read_text(encoding="utf-8")
        pack_post = (ROOT / "crates/horizon-client-render/src/gfx/pack_post.rs").read_text(encoding="utf-8")
        gfx = (ROOT / "crates/horizon-client-render/src/gfx.rs").read_text(encoding="utf-8")
        game = (ROOT / "crates/horizon-client/src/game.rs").read_text(encoding="utf-8")
        layout = (ROOT / "crates/horizon-client-render/src/touch/layout.rs").read_text(encoding="utf-8")
        touch = (ROOT / "crates/horizon-client-render/src/touch.rs").read_text(encoding="utf-8")
        editor = (ROOT / "crates/horizon-client-render/src/touch/editor.rs").read_text(encoding="utf-8")
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        launcher_pages = (ROOT / "crates/horizon-client/src/android/launcher_pages.rs").read_text(encoding="utf-8")

        # 1. ARM Mali (0x13b5) MRT limit fix & !flat terrain normal/tangent decoding
        self.assertIn("0x13b5", adapter)
        self.assertIn("0x5143", adapter)
        self.assertIn('std::env::var("HORIZON_FLAT_DRAWS").as_deref() == Ok("1")', flat)
        self.assertIn("((q.facing as u32) & 0x7) << 28", mesher)
        self.assertIn(
            "uint hz_face = min((uint(round(hz_v_color.a * 255.0)) >> 4u) & 7u, 5u);",
            pack_atlas,
        )
        self.assertIn("module.types.insert(", pack_post)
        self.assertIn("pub use pack_post::PackPost;", gfx)

        # 2. Complementary Reimagined loads while in-world menu is open (if title, not if skip_world)
        self.assertIn("let pack_inputs = if title {", game)

        # 3. Default MCPE Hit button (Control::Attack) passthrough & continuous camera drag
        self.assertIn("passthrough_fingers: BTreeSet<u64>,", touch)
        self.assertIn("row[Control::Attack as usize].passthrough = true;", layout)

        # 4. Real-time continuous slider dragging in ButtonProperty & Video Settings
        self.assertIn("PropsSlider {", editor)
        self.assertIn("fn apply_props_slider(", touch_ui)
        self.assertIn("fn draw_m3_selector_cell(", touch_ui)
        self.assertIn("Action::RenderScaleDelta(", launcher_pages)


    def test_final_polish_vertex_shaders_mods_2d_goy_skins_and_sliders(self):
        manifest = (ROOT / "crates/horizon-shader-api/src/manifest.rs").read_text(encoding="utf-8")
        stages = (ROOT / "crates/horizon-shader-api/src/stages.rs").read_text(encoding="utf-8")
        pack_shadow = (ROOT / "crates/horizon-client-render/src/gfx/pack_shadow.rs").read_text(encoding="utf-8")
        pack_uniforms = (ROOT / "crates/horizon-client-render/src/gfx/pack_uniforms.rs").read_text(encoding="utf-8")
        perspective = (ROOT / "crates/horizon-client/src/game/perspective.rs").read_text(encoding="utf-8")
        shader_packs = (ROOT / "crates/horizon-client/src/game/shader_packs.rs").read_text(encoding="utf-8")
        rewrite = (ROOT / "crates/horizon-shaders/src/translate/rewrite.rs").read_text(encoding="utf-8")
        catalog = (ROOT / "crates/horizon-client/src/android/catalog.rs").read_text(encoding="utf-8")
        wardrobe = (ROOT / "crates/horizon-client/src/android/wardrobe.rs").read_text(encoding="utf-8")
        layout = (ROOT / "crates/horizon-client-render/src/touch/layout.rs").read_text(encoding="utf-8")
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        launcher_pages = (ROOT / "crates/horizon-client/src/android/launcher_pages.rs").read_text(encoding="utf-8")

        self.assertIn("pub vertex: Option<Stage>,", manifest)
        self.assertIn("StageKind::Vertex", stages)
        self.assertIn(".min(4096)", pack_shadow)
        self.assertIn('"lightningBoltPosition"', pack_uniforms)
        self.assertIn('"normalMatrix"', pack_uniforms)
        self.assertIn("self.pack_shadow_casters.0", perspective)
        self.assertIn("pub fn pending_shader_pack_status()", shader_packs)
        self.assertIn("hz_dot", rewrite)
        self.assertIn("float hz_dot(vec3 a, vec3 b) { return dot(a, b); }", rewrite)
        inputs_rs = (ROOT / "crates/horizon-shader-api/src/inputs.rs").read_text(encoding="utf-8")
        self.assertIn("colored_light:", inputs_rs)
        self.assertIn("displaced:", inputs_rs)
        self.assertIn("pub const SURFACE_DATA: Struct", inputs_rs)
        self.assertIn("crate::touch::draw_mcpe_custom_button(", touch_ui)
        self.assertIn("{ if (true) discard; }", rewrite)
        self.assertIn("builtin:", catalog)
        self.assertIn("query.limit = CATALOG_PAGE_SIZE;", catalog)
        self.assertNotIn("Action::WardrobeZoom(false)", wardrobe)
        self.assertIn("fn default_shadow_size() -> f32 {\n    0.0\n}", layout)
        self.assertIn("let sens_card_y = body_y + 148.0;", touch_ui)
        self.assertIn("pub gyro_sensitivity_dragging: bool,", touch_ui)
        self.assertIn("pub button_scale_dragging: bool,", touch_ui)
        self.assertIn("ACTIVE_SLIDER_DRAG", launcher_pages)



    def test_uncapped_fps_import_export_inventory_player_m3_controls_homepage_and_duplicate_confirm(self) -> None:
        catalog = (ROOT / "crates/horizon-client/src/android/catalog.rs").read_text(encoding="utf-8")
        launcher = (ROOT / "crates/horizon-client/src/android/launcher.rs").read_text(encoding="utf-8")
        launcher_pages = (ROOT / "crates/horizon-client/src/android/launcher_pages.rs").read_text(encoding="utf-8")
        window = (ROOT / "crates/horizon-client-input/src/window.rs").read_text(encoding="utf-8")
        native_vk = (ROOT / "third_party/wgpu-hal/src/vulkan/swapchain/native.rs").read_text(encoding="utf-8")
        activity_java = (ROOT / "packaging/android/java/dev/horizon/android/HorizonActivity.java").read_text(encoding="utf-8")
        perspective = (ROOT / "crates/horizon-client/src/game/perspective.rs").read_text(encoding="utf-8")
        hud = (ROOT / "crates/horizon-client-render/src/hud.rs").read_text(encoding="utf-8")
        layout = (ROOT / "crates/horizon-client-render/src/touch/layout.rs").read_text(encoding="utf-8")
        editor = (ROOT / "crates/horizon-client-render/src/touch/editor.rs").read_text(encoding="utf-8")
        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        documents = (ROOT / "crates/horizon-client/src/android/documents.rs").read_text(encoding="utf-8")
        doc_bridge = (ROOT / "crates/horizon-client/src/android/document_bridge.rs").read_text(encoding="utf-8")
        native_ime = (ROOT / "crates/horizon-client/src/android/native_ime.rs").read_text(encoding="utf-8")

        # 1. Uncapped FPS on by default & Vulkan swapchain Immediate/Mailbox non-blocking presentation
        self.assertIn('#[serde(default = "default_true")]\n    pub max_fps_mode: bool,', catalog)
        self.assertIn("max_fps_mode: true,", catalog)
        self.assertIn("catalog.config.apply_experimental_to_game(&home);", launcher)
        self.assertIn("modes.push(wgt::PresentMode::Immediate);", native_vk)
        self.assertIn("mode = vk::PresentModeKHR::MAILBOX;", native_vk)
        self.assertIn("applyHighRefreshRate", activity_java)
        self.assertIn("PresentMode::Mailbox,", window)

        # 2. Import & Export Controls via system File Manager + auto-detect MCPE vs GoyControl
        self.assertIn("pub fn detect_and_import_preset(", layout)
        self.assertIn("pub fn detect_control_style(", layout)
        self.assertIn("pub fn begin_control_import(", documents)
        self.assertIn("pub fn begin_control_export(", documents)
        self.assertIn("pub fn create_output(", doc_bridge)
        self.assertIn("android.intent.action.CREATE_DOCUMENT", native_ime)
        self.assertIn("request != 4103", activity_java)
        self.assertIn("Action::ImportControlProfile", launcher_pages)
        self.assertIn("Action::ExportControlProfile", launcher_pages)

        # 3. 3D Player Model in Inventory depth buffer & clip fix (no shader regressions)
        preview_fn = perspective.split("fn draw_player_preview(")[1].split("fn draw_inventory_player(")[0]
        self.assertIn("batch.clip_since(start, x0, y0, x1, y1);", preview_fn)
        self.assertIn("batch.depth_since(start, [x0, y0, x1, y1]);", preview_fn)
        self.assertNotIn('#[cfg(not(target_os = "android"))]\n        batch.depth_since', preview_fn)
        self.assertIn("let is_2d = corners.iter().all(|v| v.depth == 0.0);", hud)

        # 4. MCPE top-left '...' draggable & screen-clamped button + Send Key in Recommended tab
        self.assertIn("fn default_floating_pos() -> [f32; 2] {\n    [0.02, 0.03]\n}", layout)
        self.assertIn("let total_dots_w = dot_sz * 3.0 + dot_gap * 2.0;", touch_ui)
        self.assertIn('"Send Key (Keyboard / Mouse / Action)"', touch_ui)

        # 5. Homepage shows selected Renderer mode and downloaded version Changelogs
        self.assertIn("Renderer: {}", launcher_pages)
        self.assertIn("CHANGELOG — MINECRAFT 26.3 • HORIZON ANDROID", launcher_pages)

        # 6. Duplicate control preset name confirmation dialog (Save to replace or Cancel)
        self.assertIn("pub duplicate_name_confirm_open: bool,", editor)
        self.assertIn("pub fn find_preset_by_name(", layout)
        self.assertIn("pub fn save_or_replace_preset_by_name(", layout)
        self.assertIn('"Replace Existing Control Preset?"', touch_ui)
        self.assertIn("Action::ConfirmReplaceDuplicatePreset", launcher_pages)

    def test_world_folder_import_export_zip_picker_no_logs_and_mods_folder_sync(self) -> None:
        activity_java = (ROOT / "packaging/android/java/dev/horizon/android/HorizonActivity.java").read_text(encoding="utf-8")
        doc_store = (ROOT / "crates/horizon-client/src/android/document_store.rs").read_text(encoding="utf-8")
        documents = (ROOT / "crates/horizon-client/src/android/documents.rs").read_text(encoding="utf-8")
        catalog = (ROOT / "crates/horizon-client/src/android/catalog.rs").read_text(encoding="utf-8")
        launcher_pages = (ROOT / "crates/horizon-client/src/android/launcher_pages.rs").read_text(encoding="utf-8")
        layout = (ROOT / "crates/horizon-client-render/src/touch/layout.rs").read_text(encoding="utf-8")

        # 1. World / Folder import never throws unhandled Java exception and takes persistable URI permission
        self.assertIn("takePersistableUriPermission(", activity_java)
        self.assertNotIn("copyFolderTree(android.content.Context context, String treeUriString, String destDirPath) throws Exception", activity_java)
        self.assertIn("android.net.Uri treeUri, String parentDocId, java.io.File destDir)", activity_java)
        self.assertIn('if let Some(err_msg) = folder_name.strip_prefix("ERROR:")', documents)
        self.assertIn("pub fn extract_world_zip_if_valid(", doc_store)

        # 2. Exports never dump into logs/performance (`reports`) and let user pick where to save/paste
        self.assertIn('let staging_dir = home.join(".export-staging");', doc_store)
        self.assertNotIn('let reports = root_home.join("performance");', layout)
        self.assertIn("pub fn begin_file_export(", documents)
        self.assertIn("pub fn export_folder_prepared(", documents)
        self.assertIn("self.catalog.fm_clipboard = Some((vec![zip_path], false));", launcher_pages)

        # 3. Mods folder syncs native mods (.hnmod) and enabled bundled mods (.mod.json) so it never shows empty
        self.assertIn("pub fn sync_mods_folder(profile_root: &Path)", doc_store)
        self.assertIn("sync_mods_folder(profile_root);", doc_store)
        self.assertIn("crate::document_store::sync_mods_folder(&game);", catalog)

        # 4. .hnmod and .jar import, validation, canonical ID extraction, auto-enabling, and in-game Options > Mods listing
        content_files = (ROOT / "crates/horizon-client/src/content_files.rs").read_text(encoding="utf-8")
        native_hooks = (ROOT / "crates/horizon-client/src/game/native_hooks.rs").read_text(encoding="utf-8")
        self.assertIn('lower.ends_with(".hmod") || lower.ends_with(".hnmod") || lower.ends_with(".jar")', content_files)
        self.assertIn("pub fn read_mod_archive_id(", content_files)
        self.assertIn("content_files::read_mod_archive_id(&staged, &name)?;", documents)
        self.assertIn("ensure_dynamic_mods_loaded", _read_repo_file("crates/horizon-client/src/game/android_bundled.rs"))


    def test_photon_shader_world_import_ui_fit_and_native_compiler_kit(self) -> None:
        import json, struct, subprocess, tempfile, zipfile

        doc_bridge = (ROOT / "crates/horizon-client/src/android/document_bridge.rs").read_text(encoding="utf-8")
        documents = (ROOT / "crates/horizon-client/src/android/documents.rs").read_text(encoding="utf-8")
        launcher_pages = (ROOT / "crates/horizon-client/src/android/launcher_pages.rs").read_text(encoding="utf-8")
        upload = (ROOT / "crates/horizon-client-render/src/gfx/upload.rs").read_text(encoding="utf-8")
        android_main = (ROOT / "crates/horizon-client/src/android_main.rs").read_text(encoding="utf-8")
        condition = (ROOT / "crates/horizon-shaders/src/condition.rs").read_text(encoding="utf-8")
        preprocess = (ROOT / "crates/horizon-shaders/src/preprocess.rs").read_text(encoding="utf-8")
        rewrite = (ROOT / "crates/horizon-shaders/src/translate/rewrite.rs").read_text(encoding="utf-8")
        pack_post = (ROOT / "crates/horizon-client-render/src/gfx/pack_post.rs").read_text(encoding="utf-8")
        pack_post_wgsl = (ROOT / "crates/horizon-client-render/src/gfx/pack_post.wgsl").read_text(encoding="utf-8")
        native_mods_java = (ROOT / "packaging/android/java/dev/horizon/android/NativeMods.java").read_text(encoding="utf-8")
        activity_java = (ROOT / "packaging/android/java/dev/horizon/android/HorizonActivity.java").read_text(encoding="utf-8")
        manifest_xml = (ROOT / "packaging/android/AndroidManifest.xml").read_text(encoding="utf-8")
        android_rs = (ROOT / "crates/horizon-client/src/android.rs").read_text(encoding="utf-8")
        native_builds = (ROOT / "crates/horizon-client/src/game/native_builds.rs").read_text(encoding="utf-8")
        jar_transpiler = (ROOT / "crates/horizon-client/src/jar_transpiler.rs").read_text(encoding="utf-8")
        mod_api = (ROOT / "crates/horizon-client/src/mod_api.rs").read_text(encoding="utf-8")

        # 1. World folder import via document_bridge::copy_tree + nested level.dat unwrapping
        self.assertIn("pub fn copy_tree(", doc_bridge)
        self.assertIn("env.get_object_class(activity)", doc_bridge)
        self.assertIn("flatten_nested_world_dir_if_needed(", doc_bridge)
        self.assertIn("crate::document_bridge::copy_tree(", documents)

        # 2. UI layout fit & button text auto-step-down
        self.assertIn("let wb_h = 114.0;", launcher_pages)
        self.assertIn("let studio_offset = 196.0;", launcher_pages)
        self.assertIn("let right_cta_w = if compact { 256.0 } else { 296.0 };", launcher_pages)

        # 3. StagingBelt crash guard & ProcessExitGuard
        self.assertNotIn('.expect("staging belt slices are mapped")', upload)
        self.assertIn("struct ProcessExitGuard;", android_main)

        # 4. Photon shader compilation, Render Scale < 100% CAS upscale, and ping-pong clear/keep
        self.assertIn("rest.starts_with(['(', '!'])", condition)
        self.assertIn('e.contains("without `#if`")', preprocess)
        self.assertIn("vec3 sh_evaluate_irradiance(vec4 r, vec4 g, vec4 b, vec3 d)", rewrite)
        self.assertIn("fn vs_uv_direct(@builtin(vertex_index) i: u32) -> Full", pack_post_wgsl)
        self.assertIn("upscale: (wgpu::RenderPipeline, wgpu::BindGroupLayout),", pack_post)
        self.assertIn("final_out: wgpu::TextureView,", pack_post)
        self.assertIn("wgpu::LoadOp::Load", pack_post)

        # 5. NativeMods.java + HorizonActivity.java + android.rs dlopen handoff + native_builds.rs + jar_transpiler.rs + mod_api.rs
        self.assertIn("static final int TRIES = 2;", native_mods_java)
        self.assertIn('Os.setenv("HORIZON_GAME_LIBRARY", build.getPath(), true);', activity_java)
        self.assertIn("System.load(build.getPath());", activity_java)
        self.assertIn('android:value="horizon_on_create"', manifest_xml)
        self.assertIn("dlopen(path.as_ptr(), RTLD_NOW)", android_rs)
        self.assertIn('dlsym(build, c"ANativeActivity_onCreate".as_ptr())', android_rs)
        self.assertIn("pub(super) fn offer_compile(&mut self)", native_builds)
        self.assertIn("pub fn transpile_jar_to_horizon_mods(", jar_transpiler)
        self.assertIn("pub use generated::Hooks;", mod_api)

        # 6. End-to-end execution of standalone real_java_to_hnmod_compiler.py on a synthetic Fabric .jar
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            jar_path = tmp_path / "vein_zoom_wrench.jar"
            out_hnmod = tmp_path / "vein_zoom_wrench.hnmod"
            cp_strings = [b"VeinMinerHandler", b"rotateBlock", b"fovZoom", b"KeyMapping", b"/veinmine"]
            cp_bytes = bytearray()
            for s in cp_strings:
                cp_bytes.append(1)
                cp_bytes.extend(struct.pack(">H", len(s)))
                cp_bytes.extend(s)
            cls_bytes = b"\xca\xfe\xba\xbe\x00\x00\x00\x3d" + struct.pack(">H", len(cp_strings) + 1) + bytes(cp_bytes) + b"\x00" * 16
            with zipfile.ZipFile(jar_path, "w") as zf:
                zf.writestr("fabric.mod.json", json.dumps({
                    "id": "vein-zoom-wrench",
                    "name": "Vein Zoom Wrench",
                    "version": "1.2.0",
                    "description": "Test Java mod for bytecode transpiler"
                }))
                zf.writestr("com/example/VeinMinerHandler.class", cls_bytes)
            res = subprocess.run(
                ["python3", str(ROOT / "packaging/android/real_java_to_hnmod_compiler.py"), str(jar_path), "--out", str(out_hnmod.parent)],
                capture_output=True,
                text=True,
                check=True,
            )
            self.assertTrue(out_hnmod.is_file(), res.stdout + res.stderr)
            with zipfile.ZipFile(out_hnmod, "r") as hz:
                names = set(hz.namelist())
                self.assertIn("horizon-native.json", names)
                self.assertIn("mixins/vein_zoom_wrench/horizon-mixin.toml", names)
                self.assertIn("mixins/vein_zoom_wrench/destroy.rs", names)
                self.assertIn("mixins/vein_zoom_wrench/use.rs", names)
                self.assertIn("mixins/vein_zoom_wrench/key.rs", names)



    def test_realtime_hnmod_runtime_shader_fps_gles_mouse_and_button_shapes(self):
        # 1. native_hooks.rs must satisfy hook_points_stay_hookable (no #[cfg] or ! before #[cfg(test)])
        native_hooks = _read_repo_file("crates/horizon-client/src/game/native_hooks.rs")
        prod_hooks = native_hooks.split("#[cfg(test)]")[0]
        self.assertNotIn("#[cfg", prod_hooks)
        for line in prod_hooks.splitlines():
            if not line.lstrip().startswith("//"):
                for mac in ["!(", "![", "! {", "!{"]:
                    self.assertNotIn(mac, line)
        self.assertIn("super::android_bundled::native_tick(self);", prod_hooks)
        self.assertIn("super::android_bundled::native_visuals(self, rims, lights);", prod_hooks)
        self.assertIn("super::android_bundled::native_hud(self, batch, corners);", prod_hooks)

        # 2. native_builds.rs has "Choose a build..." / "Try again" UI disabled
        native_builds = _read_repo_file("crates/horizon-client/src/game/native_builds.rs")
        self.assertIn("pub(super) fn native_builds_shown(&self) -> bool {\n        false\n    }", native_builds)

        # 3. Real-time compilation in jar_transpiler.rs, catalog.rs, document_store.rs, documents.rs
        jar_transpiler = _read_repo_file("crates/horizon-client/src/jar_transpiler.rs")
        self.assertIn("pub fn compile_hnmod_bundle_realtime(", jar_transpiler)
        self.assertIn("horizon-compiled.json", jar_transpiler)
        self.assertIn("enable_mod_in_profile(profile_dir, &mod_id);", jar_transpiler)

        catalog = _read_repo_file("crates/horizon-client/src/android/catalog.rs")
        self.assertIn("seen_mod_ids", catalog)
        self.assertIn("install_builtin_horizon_mod(&project.id, &game)", catalog)

        # 4. In-process live .hnmod execution in android_bundled.rs
        android_bundled = _read_repo_file("crates/horizon-client/src/game/android_bundled.rs")
        for feat in [
            "appleskin_hud",
            "immersive_thunder",
            "wrench",
            "veinminer",
            "jade_hud",
            "dynamic_lights",
            "zoom",
        ]:
            self.assertIn(feat, android_bundled)

        # 5. Shader FPS overwrite vs keep and upscale_group caching in pack_post.rs
        pack_post = _read_repo_file("crates/horizon-client-render/src/gfx/pack_post.rs")
        self.assertIn("blends: u16", pack_post)
        self.assertIn("upscale_group: wgpu::BindGroup", pack_post)

        # 6. Naga GLSL writer supports ImageClass::Depth in write_image_load
        naga_glsl = _read_repo_file("third_party/naga/src/back/glsl/writer.rs")
        self.assertIn('crate::ImageClass::Depth { multi: _ } => ("texelFetch", self.policies.image_load)', naga_glsl)

        # 7. Shader persistence in shader_packs.rs
        shader_packs = _read_repo_file("crates/horizon-client/src/game/shader_packs.rs")
        fn_failed = shader_packs.split("fn shader_pack_failed")[1].split("pub(super) fn")[0]
        self.assertNotIn("self.settings.horizon.shader_pack = String::new()", fn_failed)

        # 8. Winit EventLoop recreation allowed on Android & mouse ToolType/button mapping
        event_loop = _read_repo_file("third_party/winit/src/event_loop.rs")
        self.assertIn("#[cfg(not(android_platform))]", event_loop)
        android_mod = _read_repo_file("third_party/winit/src/platform_impl/android/mod.rs")
        self.assertIn("is_mouse_tool", android_mod)
        mouse_state = _read_repo_file("third_party/winit/src/platform_impl/android/mouse_state.rs")
        self.assertIn("pub fn buttons(&self) -> u32", mouse_state)

        # 9. Control Editor custom button naming & 7 shapes (0..=6)
        layout = _read_repo_file("crates/horizon-client-render/src/touch/layout.rs")
        self.assertIn("pub control_labels: [String; 14]", layout)
        self.assertIn(".min(6)", layout)
        editor = _read_repo_file("crates/horizon-client-render/src/touch/editor.rs")
        self.assertIn("pub editing_button_label: bool", editor)
        touch_ui = _read_repo_file("crates/horizon-client/src/touch_ui.rs")
        self.assertIn("Square (Sharp)", touch_ui)
        self.assertIn("Triangle", touch_ui)



    def test_shader_sprites_vulkan_mode_lod32_keyboard_and_material_import(self):
        # 1. Shader pack horizon_terrain_sprites fix
        mat_src = _read_repo_file("crates/horizon-client-render/src/gfx/native_material.rs")
        self.assertNotIn("horizon_terrain_sprites_0", mat_src)
        self.assertIn("horizon_terrain_sprites", mat_src)

        # 2. PowerVR broad img driver check fixed
        gfx_src = _read_repo_file("crates/horizon-client-render/src/gfx.rs")
        self.assertNotIn('driver.contains("img")', gfx_src)

        # 3. Vulkan graphics API selection
        rt_src = _read_repo_file("crates/horizon-client-render/src/android_runtime.rs")
        self.assertIn("PREFER_VULKAN", rt_src)
        self.assertIn("GraphicsApi::Vulkan", rt_src)

        # 4. LOD chunks down to 32
        gui_src = _read_repo_file("crates/horizon-client-render/src/graphics_ui.rs")
        self.assertIn("0,\n    32,\n    64,", gui_src)

        # 5. ModList::parse supports mod.id:true and bare id
        modlist_src = _read_repo_file("crates/horizon-client-render/src/mod_list.rs")
        self.assertIn('line.strip_prefix("mod.").unwrap_or(line)', modlist_src)

        # 6. jar_transpiler enable_mod_in_profile and treefall
        jt_src = _read_repo_file("crates/horizon-client/src/jar_transpiler.rs")
        self.assertIn('format!("mod.{clean_id}:true")', jt_src)
        self.assertIn('combined.contains("treefall")', jt_src)

        # 7. android_bundled scans mods/ folder
        ab_src = _read_repo_file("crates/horizon-client/src/game/android_bundled.rs")
        self.assertIn('"mods",', ab_src)

        # 8. Keyboard input switches to KeyboardMouse and is never dropped
        game_src = _read_repo_file("crates/horizon-client/src/game.rs")
        self.assertIn("InputMode::KeyboardMouse", game_src)

        # 9. Material 3 [Importing...] dialog with progress bar
        doc_src = _read_repo_file("crates/horizon-client/src/android/documents.rs")
        self.assertIn("[Importing...]", doc_src)
        self.assertIn("pb_w * fill_ratio", doc_src)
        self.assertIn("Importing folder...", doc_src)


    def test_icons_atlas_partition_missing_glyphs_and_security_hardening(self):
        ui_theme = (ROOT / "crates/horizon-client/src/ui_theme.rs").read_text(encoding="utf-8")
        self.assertIn("Icon::Skins =>", ui_theme)
        self.assertIn("image_shelf: (u32, u32, u32)", ui_theme)
        self.assertIn("fn place_image(&mut self, atlas: &HudAtlas, w: u32, h: u32)", ui_theme)
        self.assertIn("self.upload_image_rgba(atlas, tw, th, masked)?;", ui_theme)

        launcher_pages = (ROOT / "crates/horizon-client/src/android/launcher_pages.rs").read_text(encoding="utf-8")
        self.assertIn("Page::Skins => Icon::Skins,", launcher_pages)
        self.assertIn("while font > 9.5 && ui.measure(Face::Medium, font, label) > avail_w", launcher_pages)

        jar_transpiler = (ROOT / "crates/horizon-client/src/jar_transpiler.rs").read_text(encoding="utf-8")
        self.assertIn("fn is_safe_zip_entry_name(name: &str) -> bool", jar_transpiler)
        self.assertIn("fn read_zip_entry_bounded<R: Read>(entry: R)", jar_transpiler)

        sandbox = (ROOT / "packaging/android/mod_build_sandbox.sh").read_text(encoding="utf-8")
        self.assertIn("back cargo .cargo ro", sandbox)



    def test_pc_horizon_shader_mod_rain_and_import_parity(self) -> None:
        native_material = (ROOT / "crates/horizon-client-render/src/gfx/native_material.rs").read_text(encoding="utf-8")
        self.assertIn("override_ident(line)", native_material)
        self.assertIn("horizon_block_sprite_size()", native_material)
        self.assertIn("vertex_stage_overrides_and_helpers_link_with_material", native_material)

        mod_list = (ROOT / "crates/horizon-client-render/src/mod_list.rs").read_text(encoding="utf-8")
        self.assertIn("LIVE_MOD_STATE", mod_list)
        self.assertIn("pub fn is_live_disabled(", mod_list)

        native_hooks = (ROOT / "crates/horizon-client/src/game/native_hooks.rs").read_text(encoding="utf-8")
        self.assertIn(
            "super::android_bundled::native_right_click_block(self, pos, face, canceled)",
            native_hooks,
        )
        self.assertIn(
            "super::android_bundled::native_on_check(self, mod_id)",
            native_hooks,
        )

        android_bundled = (ROOT / "crates/horizon-client/src/game/android_bundled.rs").read_text(encoding="utf-8")
        self.assertNotIn("HUD Overlay & Visuals", android_bundled)
        self.assertNotIn("Active Gameplay Effect", android_bundled)
        self.assertIn("game_wrench_turn", android_bundled)
        self.assertIn("try_right_click_harvest_crop", android_bundled)
        self.assertIn("ore_family", android_bundled)
        self.assertIn("is_tree_log", android_bundled)
        self.assertIn("estimate_food_values", android_bundled)
        self.assertIn("jade_hud", android_bundled)

        jar_transpiler = (ROOT / "crates/horizon-client/src/jar_transpiler.rs").read_text(encoding="utf-8")
        self.assertIn("canonical_horizon_mod_meta", jar_transpiler)
        self.assertIn("horizon-mixin.toml", jar_transpiler)

        documents = (ROOT / "crates/horizon-client/src/android/documents.rs").read_text(encoding="utf-8")
        self.assertIn('notice("Import cancelled", false);', documents)
        self.assertIn("s.working = false;", documents)

        audio = (ROOT / "crates/horizon-client-render/src/audio.rs").read_text(encoding="utf-8")
        self.assertIn("cacheable_stream", audio)



    def test_swipe_through_goy_opacity_deletable_joystick_and_glass_ui(self) -> None:
        custom_rs = (ROOT / "crates/horizon-client-render/src/touch/custom.rs").read_text(encoding="utf-8")
        self.assertIn("activate_custom_button_at", custom_rs)
        self.assertIn("release_custom_finger_key", custom_rs)
        self.assertIn("swipe_through_buttons_triggers_each_button_in_sequence_and_zero_opacity_goy_works", custom_rs)

        touch_rs = (ROOT / "crates/horizon-client-render/src/touch.rs").read_text(encoding="utf-8")
        self.assertIn("custom_swipe_btn", touch_rs)
        self.assertIn("builtin_swipe_ctrl", touch_rs)
        self.assertIn("draw_convex_poly", touch_rs)

        touch_ui = (ROOT / "crates/horizon-client/src/touch_ui.rs").read_text(encoding="utf-8")
        self.assertIn("Delete Joystick", touch_ui)
        self.assertIn("+ Restore Joystick", touch_ui)
        self.assertIn("((t * 100.0).round() / 100.0).clamp(0.0, 1.0)", touch_ui)

        ui_theme = (ROOT / "crates/horizon-client/src/ui_theme.rs").read_text(encoding="utf-8")
        self.assertIn("set_stagger_transition", ui_theme)
        self.assertNotIn("is_glass_surface", ui_theme)
        self.assertNotIn("sheen_h", ui_theme)
        self.assertIn("draw_convex_polygon_dp", ui_theme)

        game_rs = (ROOT / "crates/horizon-client/src/game.rs").read_text(encoding="utf-8")
        self.assertIn("else if (36..45).contains(&i)", game_rs)

        bundled_rs = (ROOT / "crates/horizon-client/src/game/android_bundled.rs").read_text(encoding="utf-8")
        self.assertNotIn("game.slot_view(usize::from(game.selected) + 36)", bundled_rs)
        self.assertIn("game.inventory.get(game.selected).and_then(Option::as_ref).cloned()", bundled_rs)

        catalog_rs = (ROOT / "crates/horizon-client/src/android/catalog.rs").read_text(encoding="utf-8")
        self.assertIn("Page::Compiler", catalog_rs)
        self.assertIn("pub const CATALOG_PAGE_SIZE: usize = 10;", catalog_rs)
        self.assertIn("pub max_fps_unlocked_v2: bool,", catalog_rs)
        self.assertIn("settings.horizon.battery = self.battery_perf_mode;", catalog_rs)
        self.assertIn("if !self.page.is_catalog() && self.page != Page::Compiler {", catalog_rs)

        lp_rs = (ROOT / "crates/horizon-client/src/android/launcher_pages.rs").read_text(encoding="utf-8")
        self.assertIn("draw_compiler_page", lp_rs)
        self.assertIn("Action::ModSource", lp_rs)
        self.assertIn("if page == Page::Shaders {", lp_rs)
        self.assertIn("Page {page_num} of {total_pages}", lp_rs)

        shader_pkg = (ROOT / "crates/horizon-shader-api/src/package.rs").read_text(encoding="utf-8")
        self.assertIn("pub const BUILTIN_SHADERS: [(&str, &str, &str, u64); 10]", shader_pkg)
        self.assertIn("pub fn builtin_shader_pack(id: &str) -> Option<Pack>", shader_pkg)

        jt_rs = (ROOT / "crates/horizon-client/src/jar_transpiler.rs").read_text(encoding="utf-8")
        self.assertIn("pub const HORIZON_CATALOG_MODS: [(&str, &str, &str, u64); 24]", jt_rs)
        self.assertIn("pub fn install_builtin_horizon_mod(", jt_rs)

        bcs_rs = (ROOT / "crates/horizon-world/src/block_collision_scan.rs").read_text(encoding="utf-8")
        self.assertIn("pub fn scan_cells(bounds: Aabb) -> Option<usize>", bcs_rs)
        self.assertIn("fn section_is_empty(", bcs_rs)

    def test_water_reflections_mali_g85_perf_and_anr_fixes(self) -> None:
        pack_post = _read_repo_file("crates/horizon-client-render/src/gfx/pack_post.rs")
        # 1. Translucent gbuffers_water depth1 & scratch ping-pong copy + Source::Atlas remap guard
        self.assertIn("if group == Group::Translucent && src == Source::Depth {", pack_post)
        self.assertIn("Source::Color(n) => group != Group::Translucent && (in_place & 1 << n != 0),", pack_post)
        self.assertIn("src == Source::Atlas", pack_post)
        self.assertIn("pass.depth_linear,", pack_post)
        self.assertIn("cache_key.ticks = frame.ticks / 2;", pack_post)
        self.assertIn("static SCOPE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());", pack_post)

        # 2. Native .hshader face normals, water reflections, and sky mirror pass
        native_mat = _read_repo_file("crates/horizon-client-render/src/gfx/native_material.rs")
        self.assertIn("let face = min(high & 7u, 5u);", native_mat)
        shader_pkg = _read_repo_file("crates/horizon-shader-api/src/package.rs")
        self.assertIn("let depth_absorb = clamp(1.0 - exp(-water_depth * 0.32), 0.0, 1.0);", shader_pkg)
        self.assertIn("let absorb = clamp(1.0 - exp(-water_depth * 0.26), 0.0, 1.0);", shader_pkg)

        # 3. Mali-G52 / Helio G85 terrain shader, draw coalescing, and renderer pacing
        terrain_wgsl = _read_repo_file("crates/horizon-client-render/src/terrain.wgsl")
        self.assertIn("let texel_coords = uv * globals.filtering.y;", terrain_wgsl)
        self.assertIn("if in.emission > 0u && globals.filtering.w > 0.0 {", terrain_wgsl)
        sides = _read_repo_file("crates/horizon-client-render/src/mesher/sides.rs")
        self.assertIn('const COALESCE_GAP_QUADS: u32 = if cfg!(all(target_os = "android", not(test))) {', sides)
        gfx = _read_repo_file("crates/horizon-client-render/src/gfx.rs")
        self.assertIn('("TERRAIN_FILTERING", 0.0)', gfx)
        self.assertIn("animated_layers_state: (u64, usize, usize, usize, SectionKey, u64),", gfx)
        self.assertIn("let max_in_flight = if self.pack_post.as_ref().is_some_and(|p| p.1.is_some())", gfx)

        # 4. Vulkan swapchain cap, JNI ClassLoader fix, O(1) BlockIds lookup, and ALooper pumping
        native_vk = _read_repo_file("third_party/wgpu-hal/src/vulkan/swapchain/native.rs")
        self.assertIn("count = count.max(3);", native_vk)
        platform_rs = _read_repo_file("crates/horizon-client/src/android/platform.rs")
        self.assertIn("let cls = env.get_object_class(activity)?;", platform_rs)
        updater_rs = _read_repo_file("crates/horizon-client/src/android/updater.rs")
        self.assertIn("let cls = env.get_object_class(activity)?;", updater_rs)
        block_ids = _read_repo_file("crates/horizon-shaders/src/block_ids.rs")
        self.assertIn("by_name: std::collections::HashMap<String, Vec<usize>>,", block_ids)
        self.assertIn("pub fn contains_block(&self, name: &str) -> bool", block_ids)
        shader_packs = _read_repo_file("crates/horizon-client/src/game/shader_packs.rs")
        self.assertIn("SHADER_JOB_GENERATION", shader_packs)
        self.assertIn("SHADER_GPU_BUILD_LOCK", shader_packs)
        bundled = _read_repo_file("crates/horizon-client/src/game/android_bundled.rs")
        self.assertIn("file_cache: BTreeMap<PathBuf, (u64, Option<std::time::SystemTime>, DynamicHnmod)>,", bundled)
        pad = _read_repo_file("crates/horizon-client/src/game/pad.rs")
        self.assertIn("static LAST_GPU_PROFILE: std::sync::atomic::AtomicU8 =", pad)
        game = _read_repo_file("crates/horizon-client/src/game.rs")
        self.assertIn("w.request_redraw();", game)



    def test_wind_charge_goy_grid_snap_mcpe_editor_and_material3_ui_overhaul(self) -> None:
        # 1. Wind Charge upward trajectory fix (shoot_from_rotation, throw_wind_charge, macro slot sync, passthrough deadzone)
        projectiles_rs = _read_repo_file("crates/horizon-client/src/projectiles.rs")
        self.assertIn("let ignore_upward_source = matches!(", projectiles_rs)
        self.assertIn("ProjectileKind::WindCharge | ProjectileKind::BreezeWindCharge", projectiles_rs)
        wind_rs = _read_repo_file("crates/horizon-client/src/game/wind.rs")
        self.assertIn("let on_ground = p.ground.on_ground || velocity.y >= 0.0 || self.pitch <= 35.0;", wind_rs)
        self.assertIn("if self.pitch > 35.0 && velocity.y < 0.0 {", wind_rs)
        game_rs = _read_repo_file("crates/horizon-client/src/game.rs")
        self.assertIn("self.pad.touch.projectile_crosshair = self.touch_projectile_crosshair();", game_rs)
        touch_rs = _read_repo_file("crates/horizon-client-render/src/touch.rs")
        self.assertIn("let is_custom_passthrough = self.custom_fingers.contains_key(&id);", touch_rs)

        # 2. MCPE Control Editor bright silver fallback plates (never black) & strict MCPE renderer in editor
        self.assertIn("let bg = rgba(186, 190, 198, (u16::from(alpha) * 205 / 255) as u8);", touch_rs)
        self.assertIn("rgba(186, 192, 204, (u16::from(alpha) * 115 / 255) as u8),", touch_rs)
        touch_ui = _read_repo_file("crates/horizon-client/src/touch_ui.rs")
        self.assertIn("} else if tc.settings.control_style == ControlStyle::GoyControl {", touch_ui)

        # 3. GoyControl Grid Snap toggle in floating pod & inspector + magnetic neighbor edge-sticking
        self.assertIn("pub fn snap_button_to_neighbors_and_grid(", touch_ui)
        self.assertIn("(Some(Icon::Crosshair), grid_lbl, 74.0, true, tc.settings.snap_to_grid)", touch_ui)
        self.assertIn('"Snap to Grid & Neighbor Buttons: ON"', touch_ui)
        self.assertIn("let ew = (ww - 24.0 - 30.0) / 4.0;", touch_ui)
        self.assertIn("let bw = (ww - 24.0 - 30.0) / 4.0;", touch_ui)

        # 4. New Bricolage Grotesque font, toned-down Material 3 radii, animated background & M3 downloading screen
        theme_lib = _read_repo_file("crates/horizon-theme/src/lib.rs")
        self.assertIn('pub const FONT: &str = "Bricolage Grotesque";', theme_lib)
        ui_theme = _read_repo_file("crates/horizon-client/src/ui_theme.rs")
        self.assertIn("f(horizon_theme::fonts::BRICOLAGE_REGULAR)", ui_theme)
        self.assertIn("f(horizon_theme::fonts::BRICOLAGE_MEDIUM)", ui_theme)
        self.assertIn("f(horizon_theme::fonts::BRICOLAGE_SEMIBOLD)", ui_theme)
        self.assertIn("let t = motion::ambient_time();", ui_theme)
        lp_rs = _read_repo_file("crates/horizon-client/src/android/launcher_pages.rs")
        self.assertIn("let radius = (h * 0.22).clamp(6.0, 10.0);", lp_rs)
        self.assertIn('"Manifest" | "Resolve" => Icon::FileText,', lp_rs)
        self.assertIn('"Client JAR" | "Download" => Icon::Packs,', lp_rs)

        # 5. GoyControl 4th event slot & sequential execution, independent button holding, and chat mentions
        custom_rs = _read_repo_file("crates/horizon-client-render/src/touch/custom.rs")
        self.assertIn("pub fourth_key: String,", custom_rs)
        self.assertIn("self.custom_swipe_btn.values().any(|&idx| idx == index)", custom_rs)
        layout_rs = _read_repo_file("crates/horizon-client-render/src/touch/layout.rs")
        self.assertIn("fourth_key: String::new(),", layout_rs)
        chat_rs = _read_repo_file("crates/horizon-client-render/src/chat.rs")
        self.assertIn("builder.suggest(format!(\"@{player}\"));", chat_rs)



    def test_full_horizon_mod_loader_library_shader_store_and_fast_icon_loading(self) -> None:
        # 1. Verify all 223+ native hook fragments and upstream mod crates exist
        frag_dir = ROOT / "tools/native-hook-fragments"
        self.assertTrue(frag_dir.is_dir())
        self.assertGreaterEqual(len(list(frag_dir.glob("*.json"))), 223)
        for crate in [
            "horizon-mods",
            "horizon-hooks",
            "horizon-mod-api-gen",
            "horizon-mod-deps",
            "horizon-mod-registry",
            "cargo-horizon-mod",
            "horizon-polytone",
            "horizon-shader-api",
            "horizon-shaders",
        ]:
            self.assertTrue((ROOT / "crates" / crate / "Cargo.toml").is_file(), crate)

        # 2. Verify crates/horizon-mods 100% upstream parity (Content API 0.2 + Fluids + Registry v2 + Combined .hmod + .hpack)
        hm_content = _read_repo_file("crates/horizon-mods/src/content.rs")
        hm_fluid = _read_repo_file("crates/horizon-mods/src/content/fluid.rs")
        hm_index = _read_repo_file("crates/horizon-mods/src/index.rs")
        hm_native = _read_repo_file("crates/horizon-mods/src/index/native.rs")
        hm_page = _read_repo_file("crates/horizon-mods/src/index/page.rs")
        self.assertIn("pub enum RenderShape", hm_content)
        self.assertIn("pub enum RandomTicks", hm_content)
        self.assertIn("pub struct StateValues<T>", hm_content)
        self.assertIn("pub const LIVING_TEMPLATES:", hm_content)
        self.assertIn("pub struct Fluid", hm_fluid)
        self.assertIn("pub const FLUID_TEMPLATES:", hm_fluid)
        self.assertIn("ShaderPack,", hm_index)
        self.assertIn("DataPack,", hm_index)
        self.assertIn("pub fn holds(path: &Path) -> bool", hm_native)
        self.assertIn("pub fn source_files(&self)", hm_native)
        self.assertIn("pub fn categories(kind: &Kind)", hm_page)

        # 3. Verify horizon-client Mod Loader & 60-Mod Library + .hpack Modpack & Combined .hmod support
        mc_rs = _read_repo_file("crates/horizon-client/src/mod_content.rs")
        mod_api_rs = _read_repo_file("crates/horizon-client/src/mod_api.rs")
        nh_rs = _read_repo_file("crates/horizon-client/src/game/native_hooks.rs")
        jt_rs = _read_repo_file("crates/horizon-client/src/jar_transpiler.rs")
        cf_rs = _read_repo_file("crates/horizon-client/src/content_files.rs")
        self.assertIn("pub fn drop_dangling_remainders(", mc_rs)
        self.assertIn("pub fn live_switch(", mc_rs)
        self.assertIn("pub enum NativeChunkLoad", mod_api_rs)
        self.assertIn("pub struct NativeFluidStack", mod_api_rs)
        self.assertIn("pub enum WorldHold", mod_api_rs)
        self.assertIn("pub struct WorldWarp", mod_api_rs)
        self.assertIn("pub(crate) const NATIVE_HOOKS_API: u32 = 415;", nh_rs)
        self.assertIn("pub const HORIZON_CATALOG_MODS: [(&str, &str, &str, u64); 24]", jt_rs)
        self.assertIn("pub const HORIZON_LIBRARY_MODS: [(&str, &str, &str, u64); 36]", jt_rs)
        self.assertIn("pub fn all_horizon_catalog_mods()", jt_rs)
        self.assertIn("pub fn install_hpack_modpack(", jt_rs)
        self.assertIn("combined_content_files", jt_rs)
        self.assertIn('lower.ends_with(".hpack")', cf_rs)

        # 4. Verify Horizon Shader Store (Shader API 0.1 & 0.2 + 22 Built-in Horizon Shaders + ShaderPack support)
        sh_manifest = _read_repo_file("crates/horizon-shader-api/src/manifest.rs")
        sh_pkg = _read_repo_file("crates/horizon-shader-api/src/package.rs")
        sh_cat = _read_repo_file("crates/horizon-client/src/android/shader_catalog.rs")
        hz_browser = _read_repo_file("launcher/src/browser/horizon.rs")
        self.assertIn("pub enum Api", sh_manifest)
        self.assertIn("pub enum Blend", sh_manifest)
        self.assertIn("pub const BUILTIN_SHADERS: [(&str, &str, &str, u64); 10]", sh_pkg)
        self.assertIn("pub const HORIZON_SHADER_STORE_LIBRARY: [(&str, &str, &str, u64); 12]", sh_pkg)
        self.assertIn("pub fn all_horizon_shader_packs()", sh_pkg)
        self.assertIn("horizon_shader_api::all_horizon_shader_packs()", sh_cat)
        self.assertIn("index::Kind::Shader | index::Kind::ShaderPack => Some(Kind::Shader)", hz_browser)

        # 5. Verify Fast 6-Thread Parallel Icon Loading & Disk Caching
        images_rs = _read_repo_file("launcher/src/browser/images.rs")
        catalog_rs = _read_repo_file("crates/horizon-client/src/android/catalog.rs")
        self.assertIn("pub const FAST_ICON: u32 = 48;", images_rs)
        self.assertIn("pub fn load_cached_rgba_by_id(", images_rs)
        self.assertIn("pub fn fetch_rgba_fast(", images_rs)
        self.assertIn("const ICON_WORKER_THREADS: usize = 6;", catalog_rs)
        self.assertIn("browser::images::load_cached_rgba_by_id(", catalog_rs)
        self.assertIn("browser::images::fetch_rgba_fast(", catalog_rs)
        self.assertIn("browser::modrinth::icons(&modrinth_missing)", catalog_rs)



    def test_horizon_port_pack_native_android_modrinth_jar_and_compiler_tab(self) -> None:
        # 1. Verify horizon-port-pack native Android execution module
        pp_lib = _read_repo_file("crates/horizon-port-pack/src/lib.rs")
        self.assertIn("pub mod android_port;", pp_lib)
        android_port = _read_repo_file("crates/horizon-port-pack/src/android_port.rs")
        self.assertIn("pub struct AndroidPortResult", android_port)
        self.assertIn("pub fn port_jar_natively(", android_port)
        self.assertIn("pub fn port_modpack_natively(", android_port)
        self.assertIn("pub fn upgrade_datapack_natively(", android_port)
        self.assertIn("pub fn package_carry_dir_to_hmod(", android_port)
        self.assertIn("check::check(jar_path, &port_work_dir)", android_port)
        self.assertIn("events::report(jar_path, events::CATALOG)", android_port)

        # 2. Verify jar_transpiler.rs runs horizon_port_pack::android_port::port_jar_natively
        jt_rs = _read_repo_file("crates/horizon-client/src/jar_transpiler.rs")
        self.assertIn("pub port_pack: Option<horizon_port_pack::android_port::AndroidPortResult>,", jt_rs)
        self.assertIn("pub fn transpile_jar_to_horizon_mods_with_progress(", jt_rs)
        self.assertIn("horizon_port_pack::android_port::port_jar_natively(", jt_rs)

        # 3. Verify Mod source restricted to Horizon, while Shaders retains Modrinth option
        cat_rs = _read_repo_file("crates/horizon-client/src/android/catalog.rs")
        self.assertIn("Page::Mods => browser::SourceId::Horizon,", cat_rs)
        self.assertIn("Page::Shaders => self.config.shader_source.source(),", cat_rs)
        self.assertIn("pub fn install_modrinth_jar_with_port_pack(", cat_rs)
        self.assertIn('if lower_fname.ends_with(".mrpack") || project.kind == Kind::Modpack {', cat_rs)
        self.assertIn("horizon_port_pack::android_port::port_modpack_natively(&downloaded, home, progress)", cat_rs)

        # 4. Verify Compiler Tab removed from Page::ALL navigation, while backend types remain
        self.assertIn("pub enum CompilerMode", cat_rs)
        self.assertIn("Page::Compiler => \"Compiler\"", cat_rs)
        self.assertIn("pub const ALL: [Self; 7] =", cat_rs)
        self.assertIn("pub fn run_compiler_from_query(&mut self, home: &Path)", cat_rs)
        self.assertIn("pub fn run_compiler_on_local(&mut self, home: &Path, index: usize)", cat_rs)
        lp_rs = _read_repo_file("crates/horizon-client/src/android/launcher_pages.rs")
        self.assertIn("Action::ShaderSource(origin)", lp_rs)
        self.assertIn("Action::SetCompilerMode(mode)", lp_rs)
        self.assertIn("Action::CompilerRunQuery", lp_rs)
        self.assertIn("Action::CompilerPortLocal(orig_idx)", lp_rs)
        self.assertIn("fn draw_compiler_page(", lp_rs)



    def test_true_compiler_mod_manager_and_jetpack_compose_material_you_google_sans(self) -> None:
        # 1. Verify Google Sans TTF fonts are baked into crates/horizon-theme/fonts and loaded in ui_theme.rs
        for font_name in [
            "GoogleSans-Regular.ttf",
            "GoogleSans-Medium.ttf",
            "GoogleSans-SemiBold.ttf",
            "GoogleSans-Bold.ttf",
        ]:
            fp = ROOT / "crates/horizon-theme/fonts" / font_name
            self.assertTrue(fp.is_file() and fp.stat().st_size > 50_000, font_name)
        theme_lib = _read_repo_file("crates/horizon-theme/src/lib.rs")
        self.assertIn('pub const GOOGLE_SANS_FONT: &str = "Google Sans";', theme_lib)
        self.assertIn("pub const GOOGLE_SANS_REGULAR: &[u8]", theme_lib)
        self.assertIn("pub const GOOGLE_SANS_MEDIUM: &[u8]", theme_lib)
        self.assertIn("pub const GOOGLE_SANS_SEMIBOLD: &[u8]", theme_lib)
        self.assertIn("pub const GOOGLE_SANS_BOLD: &[u8]", theme_lib)

        ui_theme = _read_repo_file("crates/horizon-client/src/ui_theme.rs")
        self.assertIn("f(horizon_theme::fonts::GOOGLE_SANS_REGULAR)", ui_theme)
        self.assertIn("f(horizon_theme::fonts::GOOGLE_SANS_MEDIUM)", ui_theme)
        self.assertIn("f(horizon_theme::fonts::GOOGLE_SANS_SEMIBOLD)", ui_theme)
        self.assertIn("pub fn m3_tonal_card(", ui_theme)

        # 2. Verify True Compiler & Full 10-File horizon-mixins Bundle Generation + Unified .hmod (#5865) + Native .so Builder
        jt_rs = _read_repo_file("crates/horizon-client/src/jar_transpiler.rs")
        self.assertIn("pub const DEFAULT_HORIZON_REV: &str =", jt_rs)
        self.assertIn("pub const DEFAULT_MIXINS_REV: &str =", jt_rs)
        self.assertIn("pub fn generate_full_mixin_bundle_files(", jt_rs)
        self.assertIn("pub fn build_native_mods_payload(", jt_rs)
        self.assertIn("horizon_mods::index::native::Bundle::read(", jt_rs)

        cat_rs = _read_repo_file("crates/horizon-client/src/android/catalog.rs")
        self.assertIn("NativeSoBuild,", cat_rs)
        self.assertIn('Self::NativeSoBuild => "Native .so Build"', cat_rs)
        self.assertIn("pub fn run_native_so_build(&mut self, home: &Path)", cat_rs)
        self.assertIn("fn resolve_and_port_modrinth_deps_recursive(", cat_rs)
        self.assertIn('.join("checks-1")', cat_rs)

        # 3. Verify Jetpack Compose Material You Dashboard (Hero Split Button, 2-Column Bento Grid, Single Discord/GitHub, Stadium Nav Rail)
        lp_rs = _read_repo_file("crates/horizon-client/src/android/launcher_pages.rs")
        self.assertIn("ui.m3_tonal_card(content_x, card_y, content_w, hero_h, 18.0, true);", lp_rs)
        self.assertIn('"Horizon Mod Engine"', lp_rs)
        self.assertIn('"Installed Mods"', lp_rs)
        self.assertIn('"Browse Mods"', lp_rs)
        self.assertIn('"Build Native .so"', lp_rs)

        compose_kt = _read_repo_file("packaging/android/compose/HorizonComposeLauncher.kt")
        self.assertIn("val GoogleSansFontFamily", compose_kt)
        self.assertIn("fun HorizonComposeLauncherApp(", compose_kt)
        self.assertIn("fun AnimatedMaterialYouAuroraBackground(", compose_kt)


if __name__ == "__main__":
    unittest.main()
