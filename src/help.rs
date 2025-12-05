// Include the help content generated at build time
include!(concat!(env!("OUT_DIR"), "/help_content.rs"));

pub fn get_help_content() -> &'static str {
    HELP_CONTENT
}
