use ovmd::{
    config::EffectiveSource,
    render::replace_managed_block,
    source::{cache_key, classify, SourceKind},
};

#[test]
fn classifies_remote_and_raw_sources() {
    assert_eq!(classify("git@github.com:beelol/rules.git"), SourceKind::Git);
    assert_eq!(
        classify("https://raw.githubusercontent.com/beelol/rules/master/AGENTS.md"),
        SourceKind::Http
    );
}

#[test]
fn derives_readable_cache_keys() {
    let source = EffectiveSource::default();
    let key = cache_key(&source);
    assert!(key.starts_with("github.com-beelol-rules-"));
}

#[test]
fn preserves_project_specific_content() {
    let next = replace_managed_block(
        "<!-- OVERMIND:START source=old pack=universal -->\nold\n<!-- OVERMIND:END -->\n\n- keep local rule\n",
        "<!-- OVERMIND:START source=x pack=universal -->\nnew\n<!-- OVERMIND:END -->",
    )
    .unwrap();
    assert!(next.contains("- keep local rule"));
    assert!(next.contains("new"));
    assert!(!next.contains("old"));
}
