//! Car database / skin scan tests. Skipped when the game isn't installed here.

use swgp2_tool::tool::{cars, game};

fn game_dir() -> Option<std::path::PathBuf> {
    swgp2_tool::tool::settings::load()
        .game_dir
        .filter(|d| d.join(game::EXE_NAME).is_file())
        .or_else(game::detect_game_dir)
}

#[test]
fn parse_uselesslist_and_match_skins() {
    let Some(gd) = game_dir() else {
        eprintln!("game not found, skipping");
        return;
    };
    let txt = game::streaming_data_dir(&gd).join("uselesslist.txt");
    let mut cars = cars::CarEntry::parse_file(&txt);
    assert!(cars.len() >= 20, "expected the full power ranking, got {} cars", cars.len());

    cars::attach_skin_dirs(&mut cars, &game::skins_root(&gd));
    let with_skins = cars.iter().filter(|c| !c.skin_dirs.is_empty()).count();
    eprintln!("parsed {} cars, {with_skins} with skin folders", cars.len());
    assert!(with_skins * 2 >= cars.len(), "most cars should match a skin folder");

    // sanity: every entry has a plausible year and numbers
    for c in &cars {
        assert!(c.year >= 1960 && c.year <= 2026, "year out of range: {}", c.year);
        assert!(c.cv > 0 && c.top_speed > 0);
        assert!(!c.name.is_empty());
    }

    // sorting machinery
    let mut by_speed = cars.clone();
    by_speed.sort_by_key(|c| c.top_speed);
    assert!(by_speed.last().unwrap().top_speed >= by_speed.first().unwrap().top_speed);
}
