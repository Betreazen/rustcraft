fn rustcraft_stopping(game: &mut crate::game::Game<'_>) {
    crate::game::rustcraft_monorail::on_stopping(game);
    crate::game::rustcraft_workers::on_stopping(game);
    crate::game::rustcraft_base::on_stopping(game);
}
