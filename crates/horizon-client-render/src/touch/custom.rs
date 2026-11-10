//! User-added keys share drawing/hit geometry and retain per-finger ownership.
use super::*;
use layout::Placement;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomButton {
    pub key: String,
    #[serde(default)]
    pub second_key: String,
    #[serde(default)]
    pub third_key: String,
    #[serde(default)]
    pub fourth_key: String,
    #[serde(default)]
    pub custom_label: String,
    #[serde(default)]
    pub always_visible: bool,
    pub layouts: [Placement; 3],
}
impl CustomButton {
    pub fn new(key: String, index: usize) -> Self {
        Self {
            key,
            second_key: String::new(),
            third_key: String::new(),
            fourth_key: String::new(),
            custom_label: String::new(),
            always_visible: false,
            layouts: [Placement::new(
                0.68 + (index % 3) as f32 * 0.1,
                0.35 + (index / 3 % 3) as f32 * 0.12,
                0.105,
            ); 3],
        }
    }
    pub fn valid_token(token: &str) -> bool {
        token == "key.keyboard.unknown"
            || crate::keybinds::key_code(token).is_some()
            || crate::keybinds::mouse_button(token).is_some()
            || matches!(
                token,
                "mouse.scroll.up" | "mouse.scroll.down" | "mouse.virtual.toggle" | "mouse.look"
            )
            || token
                .strip_prefix("action:")
                .is_some_and(|name| crate::keybinds::MAPPINGS.iter().any(|m| m.0 == name))
    }
    pub fn is_keyboard_binding(token: &str) -> bool {
        !token.is_empty()
            && token != "key.keyboard.unknown"
            && crate::keybinds::key_code(token).is_some()
    }
    pub fn is_mouse_binding(token: &str) -> bool {
        !token.is_empty()
            && token != "key.keyboard.unknown"
            && !Self::is_keyboard_binding(token)
            && Self::valid_token(token)
    }
    /// Sets a single exclusive binding (Keyboard OR Mouse), clearing any secondary/tertiary slots.
    pub fn set_exclusive_binding(&mut self, token: String) {
        self.key = token;
        self.second_key.clear();
        self.third_key.clear();
        self.fourth_key.clear();
    }
    pub fn event_slot(&self, slot: usize) -> &str {
        match slot {
            0 => &self.key,
            1 => &self.second_key,
            2 => &self.third_key,
            _ => &self.fourth_key,
        }
    }
    pub fn set_event_slot(&mut self, slot: usize, token: String) {
        match slot {
            0 => {
                self.key = if token.is_empty() {
                    "key.keyboard.unknown".into()
                } else {
                    token
                };
            }
            1 => {
                if (self.key.is_empty() || self.key == "key.keyboard.unknown")
                    && !token.is_empty()
                    && token != "key.keyboard.unknown"
                {
                    self.key = token;
                } else {
                    self.second_key = token;
                }
            }
            2 => {
                if (self.key.is_empty() || self.key == "key.keyboard.unknown")
                    && !token.is_empty()
                    && token != "key.keyboard.unknown"
                {
                    self.key = token;
                } else {
                    self.third_key = token;
                }
            }
            _ => {
                if (self.key.is_empty() || self.key == "key.keyboard.unknown")
                    && !token.is_empty()
                    && token != "key.keyboard.unknown"
                {
                    self.key = token;
                } else {
                    self.fourth_key = token;
                }
            }
        }
    }
    pub fn token_dispatch_priority(token: &str) -> u8 {
        match token {
            "key.keyboard.1"
            | "key.keyboard.2"
            | "key.keyboard.3"
            | "key.keyboard.4"
            | "key.keyboard.5"
            | "key.keyboard.6"
            | "key.keyboard.7"
            | "key.keyboard.8"
            | "key.keyboard.9"
            | "action:key.hotbar.1"
            | "action:key.hotbar.2"
            | "action:key.hotbar.3"
            | "action:key.hotbar.4"
            | "action:key.hotbar.5"
            | "action:key.hotbar.6"
            | "action:key.hotbar.7"
            | "action:key.hotbar.8"
            | "action:key.hotbar.9"
            | "action:key.swapOffhand"
            | "key.keyboard.f"
            | "mouse.scroll.up"
            | "mouse.scroll.down" => 0,
            "key.keyboard.space"
            | "action:key.jump"
            | "key.keyboard.left.shift"
            | "key.keyboard.right.shift"
            | "action:key.sneak"
            | "key.keyboard.left.control"
            | "key.keyboard.right.control"
            | "action:key.sprint"
            | "key.keyboard.w"
            | "key.keyboard.a"
            | "key.keyboard.s"
            | "key.keyboard.d"
            | "action:key.forward"
            | "action:key.left"
            | "action:key.back"
            | "action:key.right" => 1,
            _ => 2,
        }
    }
    pub fn active_tokens(&self) -> Vec<String> {
        let mut out = Vec::new();
        for t in [&self.key, &self.second_key, &self.third_key, &self.fourth_key] {
            if !t.is_empty()
                && t != "key.keyboard.unknown"
                && Self::valid_token(t)
            {
                out.push(t.clone());
            }
        }
        out
    }
    pub fn label(&self) -> String {
        if !self.custom_label.trim().is_empty() {
            return self.custom_label.trim().to_owned();
        }
        let tokens = self.active_tokens();
        if tokens.is_empty() {
            return "Button".into();
        }
        let parts: Vec<String> = tokens.iter().map(|t| friendly_token_name(t)).collect();
        parts.join("+")
    }
}

pub fn friendly_token_name(token: &str) -> String {
    match token {
        "" | "key.keyboard.unknown" => "Unassigned".into(),
        "key.mouse.left" => "LMB (Attack)".into(),
        "key.mouse.right" => "RMB (Use)".into(),
        "key.mouse.middle" => "MMB (Pick)".into(),
        "key.mouse.4" => "Mouse 4".into(),
        "key.mouse.5" => "Mouse 5".into(),
        "mouse.scroll.up" => "Wheel Up".into(),
        "mouse.scroll.down" => "Wheel Down".into(),
        "mouse.virtual.toggle" => "Virtual Mouse".into(),
        "mouse.look" => "Mouse Look".into(),
        _ => {
            if let Some(name) = token.strip_prefix("action:") {
                let tr = crate::text::Text::translate(name, &[]).plain();
                if tr == name || tr.starts_with("key.") {
                    match name {
                        "key.attack" => "Attack / Mine".into(),
                        "key.use" => "Use / Place".into(),
                        "key.forward" => "Walk Forward".into(),
                        "key.left" => "Strafe Left".into(),
                        "key.back" => "Walk Back".into(),
                        "key.right" => "Strafe Right".into(),
                        "key.jump" => "Jump".into(),
                        "key.sneak" => "Sneak".into(),
                        "key.sprint" => "Sprint".into(),
                        "key.drop" => "Drop Item".into(),
                        "key.inventory" => "Inventory".into(),
                        "key.chat" => "Open Chat".into(),
                        "key.playerlist" => "Player List".into(),
                        "key.pickItem" => "Pick Block".into(),
                        "key.command" => "Command /".into(),
                        "key.toggleGui" => "Hide HUD (F1)".into(),
                        "key.screenshot" => "Screenshot (F2)".into(),
                        "key.togglePerspective" => "Perspective (F5)".into(),
                        "key.swapOffhand" => "Swap Hands (F)".into(),
                        "key.advancements" => "Advancements (L)".into(),
                        "key.debug.overlay" => "Debug Info (F3)".into(),
                        other => other.trim_start_matches("key.").into(),
                    }
                } else {
                    tr
                }
            } else {
                let d = crate::keybinds::display_name(token).plain();
                if d.starts_with("key.keyboard.") {
                    token.trim_start_matches("key.keyboard.").to_uppercase()
                } else {
                    d
                }
            }
        }
    }
}

pub fn choices(raw_keys: bool) -> Vec<(String, String)> {
    if raw_keys {
        crate::keybinds::available_keys()
            .iter()
            .map(|(name, _)| ((*name).into(), friendly_token_name(name)))
            .collect()
    } else {
        crate::keybinds::MAPPINGS
            .iter()
            .filter(|m| m.1 != "key.keyboard.unknown")
            .map(|(name, _, _)| {
                let tok = format!("action:{name}");
                let lbl = friendly_token_name(&tok);
                (tok, lbl)
            })
            .collect()
    }
}
impl TouchControls {
    pub(super) fn inventory_token(control: Control) -> Option<&'static str> {
        match control {
            Control::Sneak => Some("key.keyboard.left.shift"),
            Control::Attack => Some("action:key.attack"),
            Control::Use => Some("action:key.use"),
            _ => None,
        }
    }
    pub fn inventory_update(
        &mut self,
        id: u64,
        point: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        let has_swipeable = !started
            && !ended
            && self
                .custom_at(point)
                .is_some_and(|i| self.custom_placement(i).swipeable);
        if self.custom_fingers.contains_key(&id)
            || self.custom_swipe_btn.contains_key(&id)
            || (started && self.custom_at(point).is_some())
            || has_swipeable
        {
            return self.custom_update(id, point, started, ended);
        }
        if started
            && self.custom_at(point).is_none()
            && let Some(token) = self.control_at(point).and_then(Self::inventory_token)
        {
            let pressed = !self
                .custom_fingers
                .values()
                .any(|toks| toks.iter().any(|v| v == token));
            self.custom_fingers.insert(id, vec![token.into()]);
            self.fingers.insert(id, (Role::Ignore, point));
            if pressed {
                self.custom_events.push((token.into(), true));
            }
            return true;
        }
        false
    }
    /// Handles touch input for `visible_in_menu` GoyControl buttons & auxiliary controls while any
    /// menu (Main Title Menu, Inventory, Container, Pause, Options) is open.
    pub fn menu_update(&mut self, id: u64, point: [f32; 2], started: bool, ended: bool) -> bool {
        let has_swipeable = !started
            && !ended
            && self
                .custom_at(point)
                .is_some_and(|i| self.custom_placement(i).swipeable);
        let handled = if self.custom_fingers.contains_key(&id)
            || self.custom_swipe_btn.contains_key(&id)
            || (started && self.custom_at(point).is_some())
            || has_swipeable
        {
            self.custom_update(id, point, started, ended)
        } else {
            self.inventory_update(id, point, started, ended)
        };
        handled
    }
    pub fn fit_placement(&self, mut p: Placement) -> Placement {
        p.size = (p.size * self.settings.button_scale)
            .min(self.aspect())
            .min(1.0);
        let half = [p.size / self.aspect() / 2.0, p.size / 2.0];
        for (i, half) in half.into_iter().enumerate() {
            p.center[i] = p.center[i].clamp(half, 1.0 - half);
        }
        p
    }
    pub fn custom_placement(&self, index: usize) -> Placement {
        let Some(btn) = self.settings.custom_buttons.get(index) else {
            return Placement::new(0.5, 0.5, 0.105);
        };
        let mut p = btn.layouts[self.settings.mode.index()];
        if self.settings.left_handed {
            p.center[0] = 1.0 - p.center[0];
        }
        self.fit_placement(p)
    }
    pub fn custom_visible_for_state(&self, index: usize, in_menu: bool) -> bool {
        if index >= self.settings.custom_buttons.len() {
            return false;
        }
        let p = self.custom_placement(index);
        let is_goy =
            self.settings.control_style == layout::ControlStyle::GoyControl || p.use_goy_style;
        // In GoyControl, 0% opacity buttons are invisible active hitboxes (supported 0-100%).
        if p.opacity <= 0.0 && !is_goy {
            return false;
        }
        if self.editor.is_some() {
            return true;
        }
        if in_menu {
            p.visible_in_menu
        } else {
            p.visible_in_game
        }
    }
    pub fn custom_at(&self, point: [f32; 2]) -> Option<usize> {
        (0..self.settings.custom_buttons.len()).rev().find(|&i| {
            self.custom_visible_for_state(i, self.in_menu)
                && layout::contains(self.custom_placement(i).rect(self.aspect()), point)
        })
    }
    fn release_custom_finger_key(&mut self, id: u64) {
        self.passthrough_fingers.remove(&id);
        self.custom_swipe_btn.remove(&id);
        let Some(tokens) = self.custom_fingers.remove(&id) else {
            return;
        };
        for tok in tokens.into_iter().rev() {
            let still_held = self
                .custom_fingers
                .values()
                .any(|other| other.contains(&tok))
                || self.latched_custom.contains(&tok);
            if !still_held {
                self.custom_events.push((tok, false));
            }
        }
    }
    fn activate_custom_button_at(
        &mut self,
        id: u64,
        index: usize,
        point: [f32; 2],
        is_swipe: bool,
    ) -> bool {
        let Some(btn) = self.settings.custom_buttons.get(index) else {
            return false;
        };
        let active = btn.active_tokens();
        let Some(key) = active.first().cloned() else {
            return false;
        };
        let placement = self.custom_placement(index);
        self.clicked = true;
        self.custom_swipe_btn.insert(id, index);

        if placement.toggleable && key != "mouse.virtual.toggle" {
            let all_latched = active.iter().all(|t| self.latched_custom.contains(t));
            let now_on = !all_latched;
            for tok in active {
                if now_on {
                    if self.latched_custom.insert(tok.clone()) {
                        self.custom_events.push((tok, true));
                    }
                } else if self.latched_custom.remove(&tok) {
                    let held_by_finger = self
                        .custom_fingers
                        .values()
                        .any(|other| other.contains(&tok));
                    if !held_by_finger {
                        self.custom_events.push((tok, false));
                    }
                }
            }
            if !placement.passthrough || self.in_menu {
                if !is_swipe || !self.fingers.contains_key(&id) {
                    self.fingers.insert(id, (Role::Ignore, point));
                }
                return true;
            }
            self.passthrough_fingers.insert(id);
            return false;
        }

        let mut to_emit = Vec::with_capacity(active.len());
        for tok in &active {
            to_emit.push((tok.clone(), true));
        }
        self.custom_fingers.insert(id, active);
        self.custom_events.extend(to_emit);

        if placement.passthrough && !self.in_menu {
            self.passthrough_fingers.insert(id);
            if !self
                .fingers
                .get(&id)
                .is_some_and(|(r, _)| matches!(r, Role::Look { .. }))
            {
                self.fingers.insert(
                    id,
                    (
                        Role::Look {
                            start: point,
                            since: Instant::now(),
                            dragged: false,
                            held_use: false,
                            committed: false,
                        },
                        point,
                    ),
                );
            }
        } else if !self.fingers.contains_key(&id)
            || !self
                .fingers
                .get(&id)
                .is_some_and(|(r, _)| matches!(r, Role::Look { .. }))
        {
            self.fingers.insert(id, (Role::Ignore, point));
        }
        true
    }
    pub(super) fn custom_update(
        &mut self,
        id: u64,
        point: [f32; 2],
        started: bool,
        ended: bool,
    ) -> bool {
        if ended {
            self.custom_last_pos.remove(&id);
            let had_swipe = self.custom_swipe_btn.remove(&id).is_some();
            if self.custom_fingers.contains_key(&id) {
                self.release_custom_finger_key(id);
                let _ = self.fingers.remove(&id);
                return true;
            }
            if had_swipe {
                let _ = self.fingers.remove(&id);
                return true;
            }
            return false;
        }

        if started {
            self.custom_last_pos.insert(id, point);
            if let Some(index) = self.custom_at(point) {
                return self.activate_custom_button_at(id, index, point, false);
            }
            return false;
        }

        // Moving finger (`!started && !ended`): interpolate along `prev -> point` so even very fast
        // swipes across multiple `swipeable` ("Swipe: ON") buttons trigger each button in sequence!
        let prev = self.custom_last_pos.insert(id, point).unwrap_or(point);
        let dist = (point[0] - prev[0]).hypot(point[1] - prev[1]);
        let steps = ((dist / 0.015).ceil() as usize).clamp(1, 12);
        let mut swiped_any = false;

        for step in 1..=steps {
            let frac = step as f32 / steps as f32;
            let sample = [
                prev[0] + (point[0] - prev[0]) * frac,
                prev[1] + (point[1] - prev[1]) * frac,
            ];
            let hit = self.custom_at(sample);
            let prev_btn = self.custom_swipe_btn.get(&id).copied();

            // 1. Check if the finger slid out of its previous custom button
            if let Some(old_idx) = prev_btn
                && hit != Some(old_idx)
            {
                let old_swipeable = self.custom_placement(old_idx).swipeable;
                let new_swipeable = hit.is_some_and(|idx| self.custom_placement(idx).swipeable);
                if old_swipeable || new_swipeable {
                    self.release_custom_finger_key(id);
                    self.custom_swipe_btn.remove(&id);
                    swiped_any = true;
                }
            }

            // 2. Check if the finger slid onto a `swipeable` ("Swipe: ON") custom button
            if let Some(idx) = hit
                && self.custom_placement(idx).swipeable
                && self.custom_swipe_btn.get(&id) != Some(&idx)
            {
                if self.custom_fingers.contains_key(&id) {
                    self.release_custom_finger_key(id);
                }
                let _ = self.activate_custom_button_at(id, idx, sample, true);
                swiped_any = true;
            }
        }

        if self.custom_fingers.contains_key(&id) || swiped_any {
            // If Pass-through is enabled in gameplay, allow TouchControls::update to also compute camera look delta!
            if !self.in_menu
                && self
                    .fingers
                    .get(&id)
                    .is_some_and(|(r, _)| matches!(r, Role::Look { .. }))
            {
                return false;
            }
            return true;
        }
        false
    }
    pub fn held_custom_tokens(&self) -> impl Iterator<Item = &str> {
        self.custom_fingers
            .values()
            .flat_map(|v| v.iter())
            .chain(self.latched_custom.iter())
            .map(String::as_str)
    }
    pub fn custom_button_held(&self, index: usize) -> bool {
        self.custom_swipe_btn.values().any(|&idx| idx == index)
    }
    pub fn custom_button_toggled(&self, index: usize) -> bool {
        let Some(btn) = self.settings.custom_buttons.get(index) else {
            return false;
        };
        let active = btn.active_tokens();
        !active.is_empty() && active.iter().all(|tok| self.latched_custom.contains(tok))
    }
    pub fn has_pending_key_events(&self) -> bool {
        !self.custom_events.is_empty()
    }
    pub fn key_events(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.custom_events)
    }
    pub(super) fn clear_custom(&mut self) {
        self.custom_last_pos.clear();
        self.custom_swipe_btn.clear();
        let mut all_tokens: std::collections::BTreeSet<String> =
            std::mem::take(&mut self.custom_fingers)
                .into_values()
                .flatten()
                .collect();
        all_tokens.extend(std::mem::take(&mut self.latched_custom));
        self.custom_events
            .extend(all_tokens.into_iter().map(|k| (k, false)));
    }
    pub(super) fn draw_custom(&self, b: &mut HudBatch, a: &HudAtlas) {
        let hud_scale = if self.editor.is_some() {
            1.0
        } else {
            self.hud_opacity_scale.clamp(0.0, 1.0)
        };
        if hud_scale <= 0.001 && self.editor.is_none() {
            return;
        }
        for (i, button) in self.settings.custom_buttons.iter().enumerate() {
            if self.editor.is_none() && !self.custom_visible_for_state(i, self.in_menu) {
                continue;
            }
            let p = self.custom_placement(i);
            let is_goy =
                self.settings.control_style == layout::ControlStyle::GoyControl || p.use_goy_style;
            if is_goy && self.goy_rendered_by_ui && self.editor.is_none() {
                continue;
            }
            let [nx, ny, nw, nh] = p.rect(self.aspect());
            let rect = [nx * b.width, ny * b.height, nw * b.width, nh * b.height];
            let held = self.custom_button_held(i);
            let toggled = self.custom_button_toggled(i);
            let pressed = held || toggled;
            let base_opacity = if self.editor.is_some() {
                p.opacity.max(0.25)
            } else {
                p.opacity
            };
            let opacity = (base_opacity * hud_scale).clamp(0.0, 1.0);
            if opacity <= 0.001 {
                continue;
            }
            if is_goy {
                draw_goy_static_button(b, a, rect, &p, &button.label(), held, toggled, opacity);
            } else {
                text_button(
                    b,
                    a,
                    rect,
                    &button.label(),
                    pressed,
                    rgba(255, 255, 255, (opacity * 255.0).round() as u8),
                    true,
                );
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_modifier_and_mouse_hold_independently_and_cancel_cleanly() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.inventory_mode = true;
        t.settings.mode = Mode::Crosshair;
        for (id, control, token) in [
            (1, Control::Sneak, "key.keyboard.left.shift"),
            (2, Control::Attack, "action:key.attack"),
            (3, Control::Use, "action:key.use"),
        ] {
            assert!(t.visible(control));
            assert!(t.inventory_update(id, t.placement(control).center, true, false));
            assert_eq!(t.key_events(), [(token.into(), true)]);
        }
        t.clear_world_gesture();
        assert_eq!(t.held_custom_tokens().count(), 3);
        assert_eq!(t.poll_state(), PadState::default());
        assert!(t.inventory_update(2, [0.0, 0.0], false, true));
        assert_eq!(t.key_events(), [("action:key.attack".into(), false)]);
        t.clear();
        let released = t.key_events();
        assert!(released.contains(&("key.keyboard.left.shift".into(), false)));
        assert!(released.contains(&("action:key.use".into(), false)));
    }
    #[test]
    fn inventory_overlay_preserves_configured_visibility_and_placements() {
        let mut touch = TouchControls::default();
        for mode in Mode::ALL {
            touch.settings.mode = mode;
            touch.settings.button_visibility[Control::Attack as usize] = 2;
            let before: Vec<_> = Control::ALL
                .into_iter()
                .map(|c| (c, touch.visible(c), touch.placement(c).center))
                .collect();
            touch.inventory_mode = true;
            for (control, visible, center) in before {
                assert_eq!(touch.visible(control), visible);
                assert_eq!(touch.placement(control).center, center);
            }
            touch.inventory_mode = false;
        }
    }
    #[test]
    fn unassigned_button_emits_no_input() {
        let mut touch = TouchControls::default();
        touch.viewport(1920, 1080);
        touch
            .settings
            .custom_buttons
            .push(CustomButton::new("key.keyboard.unknown".into(), 0));
        let point = touch.custom_placement(0).center;
        touch.update(1, point, true, false);
        assert!(touch.key_events().is_empty());
    }
    #[test]
    fn custom_keys_hold_through_motion_and_shared_keys_release_only_after_last_finger() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.settings
            .custom_buttons
            .push(CustomButton::new("key.keyboard.z".into(), 0));
        let p = t.custom_placement(0).center;
        t.update(1, p, true, false);
        t.update(2, p, true, false);
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), true)]);
        t.update(1, [0.99, 0.1], false, false);
        assert!(t.key_events().is_empty());
        t.update(1, p, false, true);
        assert!(t.key_events().is_empty());
        t.update(2, p, false, true);
        assert_eq!(t.key_events(), [("key.keyboard.z".into(), false)]);
    }
    #[test]
    fn cancellation_releases_all_custom_keys_and_scaling_preserves_saved_sizes() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        let mut btn = CustomButton::new("key.keyboard.z".into(), 0);
        btn.set_event_slot(1, "key.keyboard.x".into());
        btn.set_event_slot(2, "key.mouse.left".into());
        t.settings.custom_buttons.push(btn);
        let stored = t.settings.custom_buttons[0].layouts[0].size;
        t.settings.button_scale = 2.0;
        assert_eq!(t.custom_placement(0).size, stored * 2.0);
        t.update(1, t.custom_placement(0).center, true, false);
        let down = t.key_events();
        assert_eq!(down.len(), 3);
        t.clear();
        let up = t.key_events();
        assert!(up.contains(&("key.keyboard.z".into(), false)));
        assert!(up.contains(&("key.keyboard.x".into(), false)));
        assert!(up.contains(&("key.mouse.left".into(), false)));
        assert_eq!(t.settings.custom_buttons[0].layouts[0].size, stored);
    }

    #[test]
    fn swipe_through_buttons_triggers_each_button_in_sequence_and_zero_opacity_goy_works() {
        let mut t = TouchControls::default();
        t.viewport(1920, 1080);
        t.settings.set_control_style(layout::ControlStyle::GoyControl);
        t.settings.custom_buttons.clear();

        // Create 3 adjacent Swipe: ON buttons at x = 0.30, 0.45, 0.60, y = 0.50
        for (i, (key, cx)) in [
            ("key.keyboard.1", 0.30f32),
            ("key.keyboard.2", 0.45f32),
            ("key.keyboard.3", 0.60f32),
        ]
        .into_iter()
        .enumerate()
        {
            let mut b = CustomButton::new(key.into(), i);
            for m in 0..3 {
                b.layouts[m].center = [cx, 0.50];
                b.layouts[m].size = 0.10;
                b.layouts[m].swipeable = true;
                b.layouts[m].use_goy_style = true;
                // Make button 2 have 0% opacity to verify 0% opacity GoyControl buttons work!
                if i == 1 {
                    b.layouts[m].opacity = 0.0;
                }
            }
            t.settings.custom_buttons.push(b);
        }

        // Start finger 1 on button 1 (0.30, 0.50)
        t.update(1, [0.30, 0.50], true, false);
        assert_eq!(t.key_events(), [("key.keyboard.1".into(), true)]);

        // Fast swipe across button 2 (0.45, 0.50, 0% opacity) all the way to button 3 (0.60, 0.50) in a single frame!
        t.update(1, [0.60, 0.50], false, false);
        let evs = t.key_events();
        assert_eq!(
            evs,
            [
                ("key.keyboard.1".into(), false),
                ("key.keyboard.2".into(), true),
                ("key.keyboard.2".into(), false),
                ("key.keyboard.3".into(), true),
            ]
        );

        // Slide off button 3 into empty space (0.80, 0.50) -> button 3 releases immediately
        t.update(1, [0.80, 0.50], false, false);
        assert_eq!(t.key_events(), [("key.keyboard.3".into(), false)]);
    }
}
