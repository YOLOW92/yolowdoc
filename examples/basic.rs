use yolowdoc::{Format, Options, convert, convert_text};

fn main() -> Result<(), yolowdoc::Error> {
    let csv_bytes = b"name,qty,price\n\"Tea, green\",3,12.5\nCoffee,2,18.0\n";
    let fmt = Format::detect(csv_bytes, Some("orders.csv"), Some("text/csv"))
        .expect("csv format detected");
    let doc = convert(csv_bytes, fmt, &Options::default())?;
    assert!(doc.markdown.contains("| Tea, green | 3 | 12.5 |"));
    println!("CSV ->\n{}", doc.markdown);

    let rtf_bytes = br#"{\rtf1\ansi{\info{\title Greeting}}Hello yolowdoc!\par}"#;
    let rtf_fmt = Format::detect(rtf_bytes, None, None).expect("rtf signature detected");
    let rtf_doc = convert(rtf_bytes, rtf_fmt, &Options::default())?;
    assert_eq!(rtf_doc.title.as_deref(), Some("Greeting"));
    assert_eq!(rtf_doc.markdown, "Hello yolowdoc!\n");
    println!("RTF -> {}", rtf_doc.markdown.trim());

    let py_src = "def greet(name: str) -> str:\n    return f\"hi {name}\"\n";
    let py_fmt = Format::from_extension("py").expect("python extension supported");
    let py_doc = convert_text(py_src, py_fmt, &Options::default())?;
    assert!(py_doc.markdown.contains("def greet"));
    println!("Python ->\n{}", py_doc.markdown);

    Ok(())
}
