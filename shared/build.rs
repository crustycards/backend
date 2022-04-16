use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = fs::create_dir("./src/proto");
    tonic_build::configure()
        .out_dir("./src/proto")
        .compile_well_known_types(true)
        .compile(
            &[
                "proto/crusty_cards_api/admin_service.proto",
                "proto/crusty_cards_api/cardpack_service.proto",
                "proto/crusty_cards_api/game_service.proto",
                "proto/crusty_cards_api/model.proto",
                "proto/crusty_cards_api/user_service.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
