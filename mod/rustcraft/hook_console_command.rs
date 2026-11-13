fn rustcraft_console_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran {
        crate::game::rustcraft_selftest::on_console_command(game, command, ran);
    }
    if !*ran {
        crate::game::rustcraft_base::on_testkit_command(game, command, ran);
    }
}
