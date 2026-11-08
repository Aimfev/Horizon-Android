// Game::use_item HEAD (not a hook point): a right click with the wrench on a
// block turns it, a sneaking one takes a machine apart. Anything else, and a
// click the wrench could do nothing with, is used as usual.
fn wrench_use(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<Result<Option<Hand>>> {
    match game.wrench_use() {
        Some(hand) => core::ops::ControlFlow::Break(Ok(hand)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
