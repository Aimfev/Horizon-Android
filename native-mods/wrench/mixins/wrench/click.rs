// native_hooks::Game::native_settings_click (a hook point): a click on the
// wrench's page changes and keeps that setting.
fn wrench_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::wrench::ID {
        game.wrench_settings_click(button);
    }
}
