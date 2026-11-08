// native_hooks::Game::native_settings (a hook point): the wrench's settings page.
fn wrench_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::wrench::ID {
        buttons.extend(game.wrench_settings());
    }
}
