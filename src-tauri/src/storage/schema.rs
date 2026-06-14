mod assistant;
mod backtest;
mod journal;
mod market;
mod research;
mod scanner;
mod trading;

const CORE_SCHEMA_GROUPS: &[&[&str]] = &[
    market::SCHEMA,
    trading::SCHEMA,
    backtest::SCHEMA,
    assistant::SCHEMA,
    journal::SCHEMA,
    scanner::SCHEMA,
    research::SCHEMA,
];

const POST_MIGRATION_SCHEMA_GROUPS: &[&[&str]] = &[trading::POST_MIGRATION_SCHEMA];

pub(super) fn core_statements() -> impl Iterator<Item = &'static str> {
    CORE_SCHEMA_GROUPS
        .iter()
        .flat_map(|group| group.iter().copied())
}

pub(super) fn post_migration_statements() -> impl Iterator<Item = &'static str> {
    POST_MIGRATION_SCHEMA_GROUPS
        .iter()
        .flat_map(|group| group.iter().copied())
}
