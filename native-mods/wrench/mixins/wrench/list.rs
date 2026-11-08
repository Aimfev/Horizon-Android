// native_hooks::Game::native_mods (a hook point): the wrench's row in Options > Mods.
fn wrench_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::wrench as wr;
    mods.push(crate::game::native_hooks::NativeMod {
        id: wr::ID.into(),
        title: format!("Wrench {}", wr::VERSION),
        description: wr::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}
