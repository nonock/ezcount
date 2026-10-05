use super::*;

const TEXT: &str = r#"<article lang="en"><h1>About</h1><p>Write to <!--contact-->.</p></article>"#;

#[test]
fn text_is_escaped_for_html() {
    assert_eq!(
        escape(r#"<a href="x">Tom & Jerry's</a>"#),
        "&lt;a href=&quot;x&quot;&gt;Tom &amp; Jerry&#39;s&lt;/a&gt;"
    );
    assert_eq!(escape("plain"), "plain");
}

#[test]
fn a_contact_is_a_link_when_it_is_an_address() {
    assert_eq!(
        contact_link("ann@example.org"),
        r#"<a href="mailto:ann@example.org">ann@example.org</a>"#
    );
    assert_eq!(
        contact_link("https://example.org/contact"),
        r#"<a href="https://example.org/contact">https://example.org/contact</a>"#
    );
    // Anything else is shown as it is, and can't bring markup or a script of its own.
    assert_eq!(contact_link("Ann, at the bar"), "Ann, at the bar");
    assert_eq!(contact_link("ann @ example"), "ann @ example");
    assert_eq!(
        contact_link(r#"javascript:alert("x")"#),
        "javascript:alert(&quot;x&quot;)"
    );
    assert_eq!(
        contact_link(r#"a@b"><script>"#),
        r#"<a href="mailto:a@b&quot;&gt;&lt;script&gt;">a@b&quot;&gt;&lt;script&gt;</a>"#
    );
}

#[test]
fn a_page_is_its_text_in_the_frame() {
    let page = framed("About <ezcount>", TEXT, None, false);
    assert!(page.starts_with("<!doctype html>"));
    assert!(page.contains("<title>About &lt;ezcount&gt;</title>"));
    assert!(page.contains("<h1>About</h1>"));
    assert!(page.contains("<body>"));
    for marker in ["<!--title-->", "<!--text-->", "<!--contact-->"] {
        assert!(!page.contains(marker), "{marker}");
    }
}

#[test]
fn a_page_shows_what_suits_the_relay() {
    let page = framed("About", TEXT, Some("ann@example.org"), true);
    assert!(page.contains("<body data-contact data-web>"));
    assert!(page.contains(r#"Write to <a href="mailto:ann@example.org">ann@example.org</a>."#));
    assert!(framed("About", TEXT, Some("ann@example.org"), false).contains("<body data-contact>"));
    assert!(framed("About", TEXT, None, true).contains("<body data-web>"));
}

#[test]
fn the_pages_come_in_both_languages() {
    for text in [
        include_str!("../privacy.html"),
        include_str!("../delete-account.html"),
    ] {
        assert_eq!(text.matches(r#"<article lang="en">"#).count(), 1);
        assert_eq!(text.matches(r#"<article lang="fr">"#).count(), 1);
        // Who to write to is the relay's to say, in each language: never an address of
        // the page's own, which every other relay would show too.
        assert_eq!(text.matches("<!--contact-->").count(), 2);
        assert!(!text.contains("mailto:"));
    }
    // The two pages lead to each other.
    assert!(include_str!("../privacy.html").contains(r#"href="delete-account""#));
    assert!(include_str!("../delete-account.html").contains(r#"href="privacy""#));
}
