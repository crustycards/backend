pub mod crusty_cards_api;

#[path = "./"]
pub mod google {
    #[path = "./google.api.rs"]
    pub mod api;

    #[path = "./google.protobuf.rs"]
    pub mod protobuf;
}
